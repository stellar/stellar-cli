use std::{
    fmt::Debug,
    path::{Path, PathBuf},
    str::FromStr,
};

use clap::{command, Parser};
use soroban_env_host::xdr::{
    BumpFootprintExpirationOp, ContractCodeEntry, ContractDataEntry, ContractEntryBodyType,
    Error as XdrError, ExtensionPoint, Hash, LedgerEntry, LedgerEntryChange, LedgerEntryData,
    LedgerFootprint, LedgerKey, LedgerKeyContractData, Memo, MuxedAccount, Operation,
    OperationBody, Preconditions, ReadXdr, ScAddress, ScSpecTypeDef, ScVal, SequenceNumber,
    SorobanResources, SorobanTransactionData, Transaction, TransactionExt, TransactionMeta,
    TransactionMetaV3, Uint256,
};
use stellar_strkey::DecodeError;

use crate::{
    commands::config,
    commands::contract::Durability,
    rpc::{self, Client},
    utils, wasm, Pwd,
};

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    /// Contract ID to which owns the data entries
    #[arg(long = "id", required_unless_present = "wasm")]
    contract_id: Option<String>,
    /// Storage key (symbols only)
    #[arg(long = "key", conflicts_with = "key_xdr")]
    key: Option<String>,
    /// Storage key (base64-encoded XDR)
    #[arg(long = "key-xdr", conflicts_with = "key")]
    key_xdr: Option<String>,
    /// Path to Wasm file of contract code to bump
    #[arg(
        long,
        conflicts_with = "contract_id",
        conflicts_with = "key",
        conflicts_with = "key_xdr"
    )]
    wasm: Option<PathBuf>,
    /// Storage entry durability
    #[arg(long, value_enum, required = true)]
    durability: Durability,

    /// Number of ledgers to extend the entries
    #[arg(long, required = true)]
    ledgers_to_expire: u32,

    #[command(flatten)]
    config: config::Args,
    #[command(flatten)]
    pub fee: crate::fee::Args,
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
    #[error("parsing key {key}: {error}")]
    CannotParseKey {
        key: String,
        error: soroban_spec_tools::Error,
    },
    #[error("parsing XDR key {key}: {error}")]
    CannotParseXdrKey { key: String, error: XdrError },
    #[error("cannot parse contract ID {0}: {1}")]
    CannotParseContractId(String, DecodeError),
    #[error(transparent)]
    Config(#[from] config::Error),
    #[error("either `--key` or `--key-xdr` are required")]
    KeyIsRequired,
    #[error("xdr processing error: {0}")]
    Xdr(#[from] XdrError),
    #[error("Ledger entry not found")]
    LedgerEntryNotFound,
    #[error("missing operation result")]
    MissingOperationResult,
    #[error(transparent)]
    Rpc(#[from] rpc::Error),
    #[error(transparent)]
    Wasm(#[from] wasm::Error),
}

impl Cmd {
    #[allow(clippy::too_many_lines)]
    pub async fn run(&self) -> Result<(), Error> {
        let expiration_ledger_seq = if self.config.is_no_network() {
            self.run_in_sandbox()?
        } else {
            self.run_against_rpc_server().await?
        };

        println!("New expiration ledger: {expiration_ledger_seq}");

        Ok(())
    }

    async fn run_against_rpc_server(&self) -> Result<u32, Error> {
        let network = self.config.get_network()?;
        tracing::trace!(?network);
        let needle = self.parse_key()?;
        let network = &self.config.get_network()?;
        let client = Client::new(&network.rpc_url)?;
        let key = self.config.key_pair()?;

        // Get the account sequence number
        let public_strkey = stellar_strkey::ed25519::PublicKey(key.public.to_bytes()).to_string();
        let account_details = client.get_account(&public_strkey).await?;
        let sequence: i64 = account_details.seq_num.into();

        let tx = Transaction {
            source_account: MuxedAccount::Ed25519(Uint256(key.public.to_bytes())),
            fee: self.fee.fee,
            seq_num: SequenceNumber(sequence + 1),
            cond: Preconditions::None,
            memo: Memo::None,
            operations: vec![Operation {
                source_account: None,
                body: OperationBody::BumpFootprintExpiration(BumpFootprintExpirationOp {
                    ext: ExtensionPoint::V0,
                    ledgers_to_expire: self.ledgers_to_expire,
                }),
            }]
            .try_into()?,
            ext: TransactionExt::V1(SorobanTransactionData {
                ext: ExtensionPoint::V0,
                resources: SorobanResources {
                    footprint: LedgerFootprint {
                        read_only: vec![needle].try_into()?,
                        read_write: vec![].try_into()?,
                    },
                    instructions: 0,
                    read_bytes: 0,
                    write_bytes: 0,
                    extended_meta_data_size_bytes: 0,
                },
                refundable_fee: 0,
            }),
        };

        let (result, meta, events) = client
            .prepare_and_send_transaction(&tx, &key, &network.network_passphrase, None)
            .await?;

        tracing::debug!(?result);
        tracing::debug!(?meta);
        if !events.is_empty() {
            tracing::debug!(?events);
        }

        // The transaction from core will succeed regardless of whether it actually found & bumped
        // the entry, so we have to inspect the result meta to tell if it worked or not.
        let TransactionMeta::V3(TransactionMetaV3 { operations, .. }) = meta else {
            return Err(Error::LedgerEntryNotFound);
        };

        // Simply check if there is exactly one entry here. We only support bumping a single
        // entry via this command (which we should fix separately, but).
        if operations.len() == 0 {
            return Err(Error::LedgerEntryNotFound);
        }

        if operations[0].changes.len() != 2 {
            return Err(Error::LedgerEntryNotFound);
        }

        match (&operations[0].changes[0], &operations[0].changes[1]) {
            (
                LedgerEntryChange::State(_),
                LedgerEntryChange::Updated(LedgerEntry {
                    data:
                        LedgerEntryData::ContractData(ContractDataEntry {
                            expiration_ledger_seq,
                            ..
                        })
                        | LedgerEntryData::ContractCode(ContractCodeEntry {
                            expiration_ledger_seq,
                            ..
                        }),
                    ..
                }),
            ) => Ok(*expiration_ledger_seq),
            _ => Err(Error::LedgerEntryNotFound),
        }
    }

    fn run_in_sandbox(&self) -> Result<u32, Error> {
        let needle = self.parse_key()?;

        // Initialize storage and host
        // TODO: allow option to separate input and output file
        let mut state = self.config.get_state()?;

        // Update all matching entries
        let mut expiration_ledger_seq = None;
        state.ledger_entries = state
            .ledger_entries
            .iter()
            .map(|(k, v)| {
                let new_k = k.as_ref().clone();
                let new_v = v.as_ref().clone();
                (
                    Box::new(new_k.clone()),
                    Box::new(if needle == new_k {
                        let (new_v, new_expiration) = bump_entry(&new_v, self.ledgers_to_expire);
                        expiration_ledger_seq = Some(new_expiration);
                        new_v
                    } else {
                        new_v
                    }),
                )
            })
            .collect::<Vec<_>>();

        self.config.set_state(&mut state)?;

        let Some(new_expiration_ledger_seq) = expiration_ledger_seq else {
            return Err(Error::LedgerEntryNotFound);
        };

        Ok(new_expiration_ledger_seq)
    }

    fn contract_id(&self) -> Result<[u8; 32], Error> {
        utils::contract_id_from_str(self.contract_id.as_ref().unwrap())
            .map_err(|e| Error::CannotParseContractId(self.contract_id.clone().unwrap(), e))
    }

    fn parse_key(&self) -> Result<LedgerKey, Error> {
        let key = if let Some(key) = &self.key {
            soroban_spec_tools::from_string_primitive(key, &ScSpecTypeDef::Symbol).map_err(|e| {
                Error::CannotParseKey {
                    key: key.clone(),
                    error: e,
                }
            })?
        } else if let Some(key) = &self.key_xdr {
            ScVal::from_xdr_base64(key).map_err(|e| Error::CannotParseXdrKey {
                key: key.clone(),
                error: e,
            })?
        } else if let Some(wasm) = &self.wasm {
            return Ok(crate::wasm::Args { wasm: wasm.clone() }.try_into()?);
        } else {
            return Err(Error::KeyIsRequired);
        };
        let contract_id = self.contract_id()?;

        Ok(LedgerKey::ContractData(LedgerKeyContractData {
            contract: ScAddress::Contract(Hash(contract_id)),
            durability: self.durability.into(),
            body_type: ContractEntryBodyType::DataEntry,
            key,
        }))
    }
}

fn bump_entry(v: &LedgerEntry, ledgers_to_expire: u32) -> (LedgerEntry, u32) {
    let mut new_v = v.clone();
    let mut new_expiration_ledger_seq = 0;
    if let LedgerEntryData::ContractData(ref mut data) = new_v.data {
        data.expiration_ledger_seq += ledgers_to_expire;
        new_expiration_ledger_seq = data.expiration_ledger_seq;
    } else if let LedgerEntryData::ContractCode(ref mut code) = new_v.data {
        code.expiration_ledger_seq += ledgers_to_expire;
        new_expiration_ledger_seq = code.expiration_ledger_seq;
    }
    (new_v, new_expiration_ledger_seq)
}
