use clap::{arg, command, Command, ArgMatches};
use soroban_rpc::client::Client;
use soroban_xdr::{ScAddress, ScVal, DecodeError};
use stellar_xdr::XdrCodec;
use crate::config::Config;
use crate::error::Result;
use clap::Parser;
use stellar_xdr::curr as xdr;

use crate::commands::{config::network, global};
use crate::config::locator;
use crate::rpc;

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub config_locator: locator::Args,

    #[command(flatten)]
    pub network: network::Args,

    /// Contract ID to get the wasm hash for
    #[arg(long)]
    pub contract_id: stellar_strkey::Contract,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Config(#[from] locator::Error),
    
    #[error(transparent)]
    Network(#[from] network::Error),
    
    #[error(transparent)]
    Rpc(#[from] rpc::Error),
}

impl Cmd {
    pub async fn run(&self, _global_args: &global::Args) -> Result<(), Error> {
        let network = self.network.get(&self.config_locator)?;
        let client = network.get_client()?;

        // Get the contract instance ledger entry
        let key = xdr::LedgerKey::ContractData(xdr::LedgerKeyContractData {
            contract: self.contract_id.clone().into(),
            key: xdr::ScVal::LedgerKeyContractInstance,
            durability: xdr::ContractDataDurability::Persistent,
        });

        let entry = client.get_ledger_entry(&key).await?;
        
        // Extract the wasm hash from the contract instance
        if let xdr::LedgerEntryData::ContractData(data) = entry.data {
            if let xdr::ScVal::ContractInstance(instance) = data.val {
                if let xdr::ContractExecutable::Wasm(hash) = instance.executable {
                    println!("{}", hex::encode(hash.0));
                    return Ok(());
                }
            }
        }

        Err(rpc::Error::InvalidResponse("Contract instance not found".into()).into())
    }
}

