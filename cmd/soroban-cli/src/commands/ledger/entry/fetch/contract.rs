use crate::commands::config::network;
use crate::commands::contract::Durability;
use crate::config::locator;
use crate::config::network::Network;
use crate::rpc;
use crate::{config, xdr};
use clap::{command, Parser};
use stellar_xdr::curr::{
    ContractDataDurability, Hash, LedgerKey, LedgerKeyContractData, Limits, ReadXdr, ScAddress,
    ScVal,
};

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    /// Contract alias or address to fetch
    pub contract: config::UnresolvedContract,

    #[command(flatten)]
    pub network: network::Args,

    #[command(flatten)]
    pub locator: locator::Args,

    //Options
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
    #[arg(long, default_value = "json")]
    pub output: OutputFormat,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Config(#[from] config::key::Error),
    #[error(transparent)]
    Locator(#[from] locator::Error),
    #[error(transparent)]
    Network(#[from] network::Error),
    #[error(transparent)]
    Rpc(#[from] rpc::Error),
    #[error(transparent)]
    Serde(#[from] serde_json::Error),
    #[error(transparent)]
    Spec(#[from] soroban_spec_tools::Error),
    #[error(transparent)]
    StellarXdr(#[from] stellar_xdr::curr::Error),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, clap::ValueEnum, Default)]
pub enum OutputFormat {
    /// JSON output of the ledger entry with parsed XDRs (one line, not formatted)
    #[default]
    Json,
    /// Formatted (multiline) JSON output of the ledger entry with parsed XDRs
    JsonFormatted,
    /// Original RPC output (containing XDRs)
    Xdr,
}

impl Cmd {
    pub async fn run(&self) -> Result<(), Error> {
        let network = self.network.get(&self.locator)?;
        let client = network.rpc_client()?;
        let mut ledger_keys = vec![];

        self.insert_keys(&network, &mut ledger_keys)?;

        match self.output {
            OutputFormat::Json => {
                let resp = client.get_full_ledger_entries(&ledger_keys).await?;
                println!("{}", serde_json::to_string(&resp)?);
            }
            OutputFormat::Xdr => {
                let resp = client.get_ledger_entries(&ledger_keys).await?;
                println!("{}", serde_json::to_string(&resp)?);
            }
            OutputFormat::JsonFormatted => {
                let resp = client.get_full_ledger_entries(&ledger_keys).await?;
                println!("{}", serde_json::to_string_pretty(&resp)?);
            }
        }

        Ok(())
    }

    fn insert_keys(
        &self,
        network: &Network,
        ledger_keys: &mut Vec<LedgerKey>,
    ) -> Result<(), Error> {
        let contract_id = self
            .contract
            .resolve_contract_id(&self.locator, &network.network_passphrase)?;
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

        Ok(())
    }
}
