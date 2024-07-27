use std::convert::Infallible;

use std::io::Write;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::{fmt::Debug, fs, io};

use clap::{arg, command, Parser};
use soroban_env_host::{
    budget::Budget,
    storage::Storage,
    xdr::{
        self, ContractCodeEntry, ContractDataDurability, ContractDataEntry, ContractExecutable,
        Error as XdrError, LedgerEntryData, LedgerKey, LedgerKeyContractCode,
        LedgerKeyContractData, ScAddress, ScContractInstance, ScVal,
    },
};

use soroban_spec::read::FromWasmError;
use stellar_strkey::DecodeError;

use crate::commands::contract::fetch::Error::{ContractIsStellarAsset, UnexpectedContractToken};
use crate::commands::{global, NetworkRunnable};
use crate::config::{
    self, locator,
    network::{self, Network},
};
use crate::{
    rpc::{self, Client},
    Pwd,
};

#[derive(Parser, Debug, Default, Clone)]
#[allow(clippy::struct_excessive_bools)]
#[group(skip)]
pub struct Cmd {
    /// Contract ID to fetch
    #[arg(long = "id", env = "STELLAR_CONTRACT_ID")]
    pub contract_id: String,
    /// Where to write output otherwise stdout is used
    #[arg(long, short = 'o')]
    pub out_file: Option<std::path::PathBuf>,
    #[command(flatten)]
    pub locator: locator::Args,
    #[command(flatten)]
    pub network: network::Args,
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
        self.locator.set_pwd(pwd);
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
    CannotWriteContractFile(PathBuf, io::Error),
    #[error("cannot parse contract ID {0}: {1}")]
    CannotParseContractId(String, DecodeError),
    #[error("network details not provided")]
    NetworkNotProvided,
    #[error(transparent)]
    Network(#[from] network::Error),
    #[error("cannot create contract directory for {0:?}")]
    CannotCreateContractDir(PathBuf),
    #[error("unexpected contract data {0:?}")]
    UnexpectedContractToken(ContractDataEntry),
    #[error(
        "cannot fetch wasm for contract because the contract is \
    a network built-in asset contract that does not have a downloadable code binary"
    )]
    ContractIsStellarAsset(),
}

impl From<Infallible> for Error {
    fn from(_: Infallible) -> Self {
        unreachable!()
    }
}

impl Cmd {
    pub async fn run(&self) -> Result<(), Error> {
        let bytes = self.get_bytes().await?;
        if let Some(out_file) = &self.out_file {
            if let Some(parent) = out_file.parent() {
                if !parent.exists() {
                    fs::create_dir_all(parent)
                        .map_err(|_| Error::CannotCreateContractDir(out_file.clone()))?;
                }
            }
            fs::write(out_file, bytes)
                .map_err(|io| Error::CannotWriteContractFile(out_file.clone(), io))
        } else {
            let stdout = std::io::stdout();
            let mut handle = stdout.lock();
            handle.write_all(&bytes)?;
            handle.flush()?;
            Ok(())
        }
    }

    pub async fn get_bytes(&self) -> Result<Vec<u8>, Error> {
        self.run_against_rpc_server(None, None).await
    }

    pub fn network(&self) -> Result<Network, Error> {
        Ok(self.network.get(&self.locator)?)
    }

    fn contract_id(&self) -> Result<[u8; 32], Error> {
        let network = self.network()?;
        Ok(self
            .locator
            .resolve_contract_id(&self.contract_id, &network.network_passphrase)?
            .0)
    }
}

#[async_trait::async_trait]
impl NetworkRunnable for Cmd {
    type Error = Error;
    type Result = Vec<u8>;
    async fn run_against_rpc_server(
        &self,
        _args: Option<&global::Args>,
        config: Option<&config::Args>,
    ) -> Result<Vec<u8>, Error> {
        let network = config.map_or_else(|| self.network(), |c| Ok(c.get_network()?))?;
        tracing::trace!(?network);
        let contract_id = self.contract_id()?;
        let client = Client::new(&network.rpc_url)?;
        client
            .verify_network_passphrase(Some(&network.network_passphrase))
            .await?;
        let data_entry = client.get_contract_data(&contract_id).await?;
        if let ScVal::ContractInstance(contract) = &data_entry.val {
            return match &contract.executable {
                ContractExecutable::Wasm(hash) => {
                    Ok(client.get_remote_wasm_from_hash(&hash).await?)
                }
                ContractExecutable::StellarAsset => Err(ContractIsStellarAsset()),
            };
        }
        return Err(UnexpectedContractToken(data_entry));
    }
}

pub fn get_contract_wasm_from_storage(
    storage: &mut Storage,
    contract_id: [u8; 32],
) -> Result<Vec<u8>, FromWasmError> {
    let key = LedgerKey::ContractData(LedgerKeyContractData {
        contract: ScAddress::Contract(contract_id.into()),
        key: ScVal::LedgerKeyContractInstance,
        durability: ContractDataDurability::Persistent,
    });
    match storage.get(&key.into(), &Budget::default()) {
        Ok(rc) => match rc.as_ref() {
            xdr::LedgerEntry {
                data:
                    LedgerEntryData::ContractData(ContractDataEntry {
                        val: ScVal::ContractInstance(ScContractInstance { executable, .. }),
                        ..
                    }),
                ..
            } => match executable {
                ContractExecutable::Wasm(hash) => {
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
                ContractExecutable::StellarAsset => todo!(),
            },
            _ => Err(FromWasmError::NotFound),
        },
        _ => Err(FromWasmError::NotFound),
    }
}
