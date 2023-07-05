use std::{fmt::Debug, path::Path, str::FromStr};

use clap::{command, Parser};
use soroban_env_host::xdr::{
    ContractDataDurability, ContractEntryBodyType, Error as XdrError, ExtensionPoint, Hash,
    LedgerFootprint, LedgerKey, LedgerKeyContractData, Memo, MuxedAccount, Operation,
    OperationBody, Preconditions, ReadXdr, RestoreFootprintOp, ScAddress, ScSpecTypeDef, ScVal,
    SequenceNumber, SorobanResources, SorobanTransactionData, Transaction, TransactionExt, Uint256,
};
use stellar_strkey::DecodeError;

use crate::{
    commands::config::{self, locator},
    rpc::{self, Client},
    utils, Pwd,
};

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    /// Contract ID to which owns the data entries
    #[arg(long = "id")]
    contract_id: String,
    /// Storage key (symbols only)
    #[arg(long = "key", required_unless_present = "key_xdr")]
    key: Vec<String>,
    /// Storage key (base64-encoded XDR)
    #[arg(long = "key-xdr", required_unless_present = "key")]
    key_xdr: Vec<String>,

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
    #[error(transparent)]
    Locator(#[from] locator::Error),
    #[error("missing operation result")]
    MissingOperationResult,
    #[error(transparent)]
    Rpc(#[from] rpc::Error),
}

impl Cmd {
    #[allow(clippy::too_many_lines)]
    pub async fn run(&self) -> Result<(), Error> {
        if self.config.is_no_network() {
            self.run_in_sandbox()
        } else {
            self.run_against_rpc_server().await
        }
    }

    async fn run_against_rpc_server(&self) -> Result<(), Error> {
        let network = self.config.get_network()?;
        tracing::trace!(?network);
        let contract_id = self.contract_id()?;
        let entry_keys = self.parse_keys(contract_id)?;
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
            seq_num: SequenceNumber(sequence),
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
                    extended_meta_data_size_bytes: 0,
                },
                refundable_fee: 0,
            }),
        };

        let (result, _meta, events) = client
            .prepare_and_send_transaction(&tx, &key, &network.network_passphrase, None)
            .await?;

        tracing::debug!(?result);
        if !events.is_empty() {
            tracing::debug!(?events);
        }

        Ok(())
    }

    fn run_in_sandbox(&self) -> Result<(), Error> {
        // TODO: Implement this. This means we need to store ledger entries somewhere, and handle
        // eviction, and restoration with that evicted state store.
        todo!("Restoring ledger entries is not supported in the local sandbox mode");
    }

    fn contract_id(&self) -> Result<[u8; 32], Error> {
        utils::contract_id_from_str(&self.contract_id)
            .map_err(|e| Error::CannotParseContractId(self.contract_id.clone(), e))
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
            return Err(Error::KeyIsRequired);
        };

        Ok(keys
            .iter()
            .map(|key| {
                LedgerKey::ContractData(LedgerKeyContractData {
                    contract: ScAddress::Contract(Hash(contract_id)),
                    durability: ContractDataDurability::Persistent,
                    body_type: ContractEntryBodyType::DataEntry,
                    key: key.clone(),
                })
            })
            .collect())
    }
}
