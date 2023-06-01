use std::convert::Infallible;

use std::io::Write;
use std::path::{Path, PathBuf};
use std::println;
use std::str::FromStr;
use std::{fmt::Debug, fs, io, rc::Rc};

use clap::{arg, command, Parser};
use hex::FromHexError;
use soroban_env_host::{
    budget::Budget,
    storage::Storage,
    xdr::{
        self, AccountId, ContractCodeEntry, ContractDataEntry, Error as XdrError, LedgerEntryData,
        LedgerKey, LedgerKeyAccount, LedgerKeyContractCode, LedgerKeyContractData, PublicKey,
        ReadXdr, ScContractExecutable, ScVal, Uint256,
    },
};

use soroban_spec::read::FromWasmError;

use super::super::config::{self, locator};
use crate::{
    rpc::{self, Client},
    utils::{self, default_account_ledger_entry},
    Pwd,
};

#[derive(Parser, Debug, Default, Clone)]
#[allow(clippy::struct_excessive_bools)]
#[group(skip)]
pub struct Cmd {
    /// Contract ID to invoke
    #[arg(long = "id", env = "SOROBAN_CONTRACT_ID")]
    pub contract_id: String,
    /// WASM file of the contract to invoke (if using sandbox will deploy this file)
    #[arg(long)]
    pub wasm: Option<std::path::PathBuf>,
    #[command(flatten)]
    pub config: config::Args,
    #[arg(long, short = 'o')]
    pub out_file: Option<std::path::PathBuf>,
}

impl FromStr for Cmd {
    type Err = clap::error::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use clap::{CommandFactory, FromArgMatches};
        Self::from_arg_matches_mut(&mut Self::command().get_matches_from(s.split_whitespace()))
    }
}

impl Pwd for Cmd {
    fn set_pwd(&mut self, pwd: &Path) {
        self.config.set_pwd(pwd);
    }
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Rpc(#[from] rpc::Error),
    #[error(transparent)]
    Config(#[from] config::Error),
    #[error(transparent)]
    Locator(#[from] locator::Error),
    #[error(transparent)]
    Xdr(#[from] XdrError),
    #[error(transparent)]
    Spec(#[from] soroban_spec::read::FromWasmError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("missing result")]
    MissingResult,
    #[error("unexpected contract code data type: {0:?}")]
    UnexpectedContractCodeDataType(LedgerEntryData),
    #[error("reading file {0:?}: {1}")]
    CannotReadContractFile(PathBuf, io::Error),
    #[error("cannot parse contract ID {0}: {1}")]
    CannotParseContractId(String, FromHexError),
}

impl From<Infallible> for Error {
    fn from(_: Infallible) -> Self {
        unreachable!()
    }
}

impl Cmd {
    pub async fn run(&self) -> Result<(), Error> {
        let bytes = if self.config.is_no_network() {
            self.run_in_sandbox()
        } else {
            self.run_against_rpc_server().await
        }?;
        if let Some(out_file) = &self.out_file {
            fs::write(out_file, bytes)
                .map_err(|io| Error::CannotReadContractFile(out_file.clone(), io))
        } else {
            let stdout = std::io::stdout();
            let mut handle = stdout.lock();
            handle.write_all(&bytes)?;
            handle.flush()?;
            Ok(())
        }
    }

    pub async fn run_against_rpc_server(&self) -> Result<Vec<u8>, Error> {
        let network = self.config.get_network()?;
        tracing::trace!(?network);
        let contract_id = self.contract_id()?;
        let network = &self.config.get_network()?;
        let client = Client::new(&network.rpc_url)?;

        // async closures are not yet stable
        get_remote_wasm(&client, &contract_id).await
    }

    pub fn run_in_sandbox(&self) -> Result<Vec<u8>, Error> {
        let contract_id = self.contract_id()?;
        // Initialize storage and host
        // TODO: allow option to separate input and output file
        let mut state = self.config.get_state()?;

        // Create source account, adding it to the ledger if not already present.
        let source_account = AccountId(PublicKey::PublicKeyTypeEd25519(Uint256(
            self.config.key_pair()?.public.to_bytes(),
        )));
        let source_account_ledger_key = LedgerKey::Account(LedgerKeyAccount {
            account_id: source_account.clone(),
        });
        if !state
            .ledger_entries
            .iter()
            .any(|(k, _)| **k == source_account_ledger_key)
        {
            state.ledger_entries.push((
                Box::new(source_account_ledger_key),
                Box::new(default_account_ledger_entry(source_account)),
            ));
        }

        let snap = Rc::new(state.clone());
        let mut storage = Storage::with_recording_footprint(snap);
        Ok(get_contract_wasm_from_storage(&mut storage, contract_id)?)
    }

    pub fn read_wasm(&self) -> Result<Option<Vec<u8>>, Error> {
        Ok(if let Some(wasm) = self.wasm.as_ref() {
            Some(fs::read(wasm).map_err(|e| Error::CannotReadContractFile(wasm.clone(), e))?)
        } else {
            None
        })
    }

    fn contract_id(&self) -> Result<[u8; 32], Error> {
        utils::id_from_str(&self.contract_id)
            .map_err(|e| Error::CannotParseContractId(self.contract_id.clone(), e))
    }
}

async fn get_remote_wasm(client: &Client, contract_id: &[u8; 32]) -> Result<Vec<u8>, Error> {
    // Get the contract from the network
    let contract_key = LedgerKey::ContractData(LedgerKeyContractData {
        contract_id: xdr::Hash(*contract_id),
        key: ScVal::LedgerKeyContractExecutable,
    });
    let contract_ref = client.get_ledger_entries(Vec::from([contract_key])).await?;
    if contract_ref.entries.is_empty() {
        return Err(Error::MissingResult);
    }
    let contract_ref_entry = &contract_ref.entries[0];
    Ok(
        match LedgerEntryData::from_xdr_base64(&contract_ref_entry.xdr)? {
            LedgerEntryData::ContractData(ContractDataEntry {
                val: ScVal::ContractExecutable(ScContractExecutable::WasmRef(hash)),
                ..
            }) => {
                let code_key = LedgerKey::ContractCode(LedgerKeyContractCode { hash });
                let contract_data = client.get_ledger_entries(Vec::from([code_key])).await?;
                if contract_data.entries.is_empty() {
                    return Err(Error::MissingResult);
                }
                let contract_data_entry = &contract_data.entries[0];
                match LedgerEntryData::from_xdr_base64(&contract_data_entry.xdr)? {
                    LedgerEntryData::ContractCode(ContractCodeEntry { code, .. }) => code.into(),
                    scval => return Err(Error::UnexpectedContractCodeDataType(scval)),
                }
            }
            scval => return Err(Error::UnexpectedContractCodeDataType(scval)),
        },
    )
}

pub fn get_contract_wasm_from_storage(
    storage: &mut Storage,
    contract_id: [u8; 32],
) -> Result<Vec<u8>, FromWasmError> {
    let key = LedgerKey::ContractData(LedgerKeyContractData {
        contract_id: contract_id.into(),
        key: ScVal::LedgerKeyContractExecutable,
    });
    match storage.get(&key.into(), &Budget::default()) {
        Ok(rc) => match rc.as_ref() {
            xdr::LedgerEntry {
                data:
                    LedgerEntryData::ContractData(ContractDataEntry {
                        val: ScVal::ContractExecutable(c),
                        ..
                    }),
                ..
            } => match c {
                ScContractExecutable::WasmRef(hash) => {
                    if let Ok(rc) = storage.get(
                        &LedgerKey::ContractCode(LedgerKeyContractCode { hash: hash.clone() })
                            .into(),
                        &Budget::default(),
                    ) {
                        match rc.as_ref() {
                            xdr::LedgerEntry {
                                data: LedgerEntryData::ContractCode(ContractCodeEntry { code, .. }),
                                ..
                            } => Ok(code.to_vec()),
                            _ => Err(FromWasmError::NotFound),
                        }
                    } else {
                        Err(FromWasmError::NotFound)
                    }
                }
                ScContractExecutable::Token => todo!(),
            },
            _ => Err(FromWasmError::NotFound),
        },
        _ => Err(FromWasmError::NotFound),
    }
}
