use std::{
    fmt::Debug,
    path::{Path, PathBuf},
    str::FromStr,
};

use clap::{command, Parser};
use soroban_env_host::xdr::{
    ContractDataDurability, Error as XdrError, ExpirationEntry, ExtensionPoint, Hash, LedgerEntry,
    LedgerEntryChange, LedgerEntryData, LedgerFootprint, LedgerKey, LedgerKeyContractCode,
    LedgerKeyContractData, Memo, MuxedAccount, Operation, OperationBody, OperationMeta,
    Preconditions, ReadXdr, RestoreFootprintOp, ScAddress, ScSpecTypeDef, ScVal, SequenceNumber,
    SorobanResources, SorobanTransactionData, Transaction, TransactionExt, TransactionMeta,
    TransactionMetaV3, Uint256,
};
use stellar_strkey::DecodeError;

use crate::{
    commands::config::{self, locator},
    rpc::{self, Client},
    utils, wasm, Pwd,
};

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    /// Contract ID to which owns the data entries.
    /// If no keys provided the Contract's instance will be restored
    #[arg(
        long = "id",
        required_unless_present = "wasm",
        required_unless_present = "wasm_hash"
    )]
    pub contract_id: Option<String>,
    /// Storage key (symbols only)
    #[arg(long = "key")]
    pub key: Vec<String>,
    /// Storage key (base64-encoded XDR)
    #[arg(long = "key-xdr")]
    pub key_xdr: Vec<String>,
    /// Path to Wasm file of contract code to restore
    #[arg(
        long,
        conflicts_with = "key",
        conflicts_with = "key_xdr",
        conflicts_with = "contract_id",
        conflicts_with = "wasm_hash"
    )]
    pub wasm: Option<PathBuf>,

    /// Hash of contract code to restore
    #[arg(
        long = "wasm-hash",
        conflicts_with = "key",
        conflicts_with = "key_xdr",
        conflicts_with = "contract_id",
        conflicts_with = "wasm"
    )]
    pub wasm_hash: Option<String>,

    #[command(flatten)]
    pub config: config::Args,
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
    #[error(transparent)]
    Locator(#[from] locator::Error),
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

    pub async fn run_against_rpc_server(&self) -> Result<u32, Error> {
        let network = self.config.get_network()?;
        tracing::trace!(?network);
        let entry_keys = if let Some(wasm) = &self.wasm {
            vec![crate::wasm::Args { wasm: wasm.clone() }.try_into()?]
        } else if let Some(wasm_hash) = &self.wasm_hash {
            vec![LedgerKey::ContractCode(LedgerKeyContractCode {
                hash: Hash(
                    utils::contract_id_from_str(wasm_hash)
                        .map_err(|e| Error::CannotParseContractId(wasm_hash.clone(), e))?,
                ),
            })]
        } else {
            let contract_id = self.contract_id()?;
            self.parse_keys(contract_id)?
        };
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
                body: OperationBody::RestoreFootprint(RestoreFootprintOp {
                    ext: ExtensionPoint::V0,
                }),
            }]
            .try_into()?,
            ext: TransactionExt::V1(SorobanTransactionData {
                ext: ExtensionPoint::V0,
                resources: SorobanResources {
                    footprint: LedgerFootprint {
                        read_only: vec![].try_into()?,
                        read_write: entry_keys.try_into()?,
                    },
                    instructions: 0,
                    read_bytes: 0,
                    write_bytes: 0,
                },
                refundable_fee: 0,
            }),
        };

        let (result, meta, events) = client
            .prepare_and_send_transaction(&tx, &key, &[], &network.network_passphrase, None, None)
            .await?;

        tracing::trace!(?result);
        tracing::trace!(?meta);
        if !events.is_empty() {
            tracing::info!("Events:\n {events:#?}");
        }

        // The transaction from core will succeed regardless of whether it actually found &
        // restored the entry, so we have to inspect the result meta to tell if it worked or not.
        let TransactionMeta::V3(TransactionMetaV3 { operations, .. }) = meta else {
            return Err(Error::LedgerEntryNotFound);
        };
        tracing::debug!("Operations:\nlen:{}\n{operations:#?}", operations.len());

        // Simply check if there is exactly one entry here. We only support bumping a single
        // entry via this command (which we should fix separately, but).
        if operations.len() == 0 {
            return Err(Error::LedgerEntryNotFound);
        }

        if operations.len() != 1 {
            tracing::warn!(
                "Unexpected number of operations: {}. Currently only handle one.",
                operations[0].changes.len()
            );
        }
        parse_operations(&operations).ok_or(Error::MissingOperationResult)
    }

    pub fn run_in_sandbox(&self) -> Result<u32, Error> {
        // TODO: Implement this. This means we need to store ledger entries somewhere, and handle
        // eviction, and restoration with that evicted state store.
        todo!("Restoring ledger entries is not supported in the local sandbox mode");
    }

    fn contract_id(&self) -> Result<[u8; 32], Error> {
        utils::contract_id_from_str(self.contract_id.as_ref().unwrap())
            .map_err(|e| Error::CannotParseContractId(self.contract_id.clone().unwrap(), e))
    }

    fn parse_keys(&self, contract_id: [u8; 32]) -> Result<Vec<LedgerKey>, Error> {
        let mut keys: Vec<ScVal> = vec![];
        for key in &self.key {
            keys.push(
                soroban_spec_tools::from_string_primitive(key, &ScSpecTypeDef::Symbol).map_err(
                    |e| Error::CannotParseKey {
                        key: key.clone(),
                        error: e,
                    },
                )?,
            );
        }
        for key in &self.key_xdr {
            keys.push(
                ScVal::from_xdr_base64(key).map_err(|e| Error::CannotParseXdrKey {
                    key: key.clone(),
                    error: e,
                })?,
            );
        }

        if keys.is_empty() {
            keys.push(ScVal::LedgerKeyContractInstance);
        };

        Ok(keys
            .iter()
            .map(|key| {
                LedgerKey::ContractData(LedgerKeyContractData {
                    contract: ScAddress::Contract(Hash(contract_id)),
                    durability: ContractDataDurability::Persistent,
                    key: key.clone(),
                })
            })
            .collect())
    }
}

fn parse_operations(ops: &[OperationMeta]) -> Option<u32> {
    ops.get(0).and_then(|op| {
        op.changes.iter().find_map(|entry| match entry {
            LedgerEntryChange::Updated(LedgerEntry {
                data:
                    LedgerEntryData::Expiration(ExpirationEntry {
                        expiration_ledger_seq,
                        ..
                    }),
                ..
            })
            | LedgerEntryChange::Created(LedgerEntry {
                data:
                    LedgerEntryData::Expiration(ExpirationEntry {
                        expiration_ledger_seq,
                        ..
                    }),
                ..
            }) => Some(*expiration_ledger_seq),
            _ => None,
        })
    })
}
