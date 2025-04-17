use std::fmt::Debug;

use clap::{command, Parser};
use stellar_xdr::curr::{ContractDataDurability, Hash, LedgerKey, LedgerKeyAccount, LedgerKeyContractData, Limits, MuxedAccount, ReadXdr, ScAddress, ScVal};
use crate::commands::config::network;
use crate::{config, xdr};
use crate::config::{locator};
use crate::{
    rpc::{self},
};
use crate::commands::contract::Durability;
use crate::commands::ledger::entry::get::Error::{ContractRequired, EmptyKeys};

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub network: network::Args,

    #[command(flatten)]
    pub locator: locator::Args,

    /// Name of identity to lookup, default is test identity
    #[arg(long)]
    pub account: Option<String>,

    /// If identity is a seed phrase use this hd path, default is 0
    #[arg(long)]
    pub hd_path: Option<usize>,


    #[arg(long = "id", env = "STELLAR_CONTRACT_ID")]
    pub contract_id: Option<config::UnresolvedContract>,

    /// Storage entry durability
    #[arg(long, value_enum, default_value = "persistent")]
    pub durability: Durability,

    /// Storage key (symbols only)
    #[arg(long = "key")]
    pub key: Option<Vec<String>>,
    /// Storage key (base64-encoded XDR)
    #[arg(long = "key-xdr")]
    pub key_xdr: Option<Vec<String>>,

    /// Format of the output
    #[arg(long, default_value = "original")]
    pub output: OutputFormat,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Config(#[from] config::key::Error),
    #[error(transparent)]
    Locator(#[from] locator::Error),
    #[error(transparent)]
    Rpc(#[from] rpc::Error),
    #[error(transparent)]
    Network(#[from] network::Error),
    #[error(transparent)]
    Serde(#[from] serde_json::Error),
    #[error(transparent)]
    StellarXdr(#[from] stellar_xdr::curr::Error),
    #[error(transparent)]
    Spec(#[from] soroban_spec_tools::Error),
    #[error("at least one key must be provided")]
    EmptyKeys,
    #[error("contract id is required but was not provided")]
    ContractRequired,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, clap::ValueEnum, Default)]
pub enum OutputFormat {
    /// Original RPC output (containing XDRs)
    #[default]
    Original,
    /// JSON output of the ledger entry with parsed XDRs (one line, not formatted)
    Json,
    /// Formatted (multiline) JSON output of the ledger entry with parsed XDRs
    JsonFormatted,
}


impl Cmd {
    pub async fn run(&self) -> Result<(), Error> {
        let network = self.network.get(&self.locator)?;
        let client = network.rpc_client()?;
        let mut ledger_keys = vec![];

        if let Some(contract_id) = &self.contract_id {
            let contract_id = contract_id.resolve_contract_id(&self.locator, &network.network_passphrase)?;

            let contract_address_arg = ScAddress::Contract(Hash(contract_id.0));

            if let Some(keys) = &self.key {
                for key in keys {
                    let key = LedgerKey::ContractData(LedgerKeyContractData {
                        contract: contract_address_arg.clone(),
                        key: soroban_spec_tools::from_string_primitive(
                            key,
                            &xdr::ScSpecTypeDef::Symbol,
                        )?,
                        durability: ContractDataDurability::Persistent,
                    });

                    ledger_keys.push(key);
                }
            }

            if let Some(keys) = &self.key_xdr {
                for key in keys {
                    let key = LedgerKey::ContractData(LedgerKeyContractData {
                        contract: contract_address_arg.clone(),
                        key: ScVal::from_xdr_base64(key, Limits::none())?,
                        durability: ContractDataDurability::Persistent,
                    });

                    ledger_keys.push(key);
                }
            }
        } else if self.key.is_some() || self.key_xdr.is_some() {
            return Err(ContractRequired)
        }

        if let Some(acc) = &self.account {
            let acc = self.muxed_account(acc)?;
            let key = LedgerKey::Account(LedgerKeyAccount { account_id: acc.account_id() });
            ledger_keys.push(key);
        }


        if ledger_keys.is_empty() {
            return Err(EmptyKeys);
        }

        match self.output {
            OutputFormat::Original => {
                let resp = client.get_ledger_entries(&ledger_keys).await?;
                println!("{}", serde_json::to_string(&resp)?);
            }
            OutputFormat::Json => {
                let resp = client.get_full_ledger_entries(&ledger_keys).await?;
                println!("{}", serde_json::to_string(&resp)?);
            }
            OutputFormat::JsonFormatted => {
                let resp = client.get_full_ledger_entries(&ledger_keys).await?;
                println!("{}", serde_json::to_string_pretty(&resp)?);
            }
        }


        return Ok(());
    }

    fn muxed_account(&self, account: &str) -> Result<MuxedAccount, Error> {
        Ok(self
            .locator
            .read_identity(account)?
            .muxed_account(self.hd_path)?)
    }
}
