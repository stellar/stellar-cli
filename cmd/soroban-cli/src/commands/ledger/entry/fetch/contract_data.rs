use super::args::Args;
use crate::{
    commands::contract::Durability,
    config::{self, locator},
    xdr::{
        self, ContractDataDurability, ContractId, Hash, LedgerKey, LedgerKeyContractData, Limits,
        ReadXdr, ScAddress, ScVal,
    },
};
use clap::{command, Parser};

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    /// Contract alias or address to fetch
    #[arg(long)]
    pub contract: config::UnresolvedContract,

    #[command(flatten)]
    pub args: Args,

    //Options
    /// Storage entry durability
    #[arg(long, value_enum, default_value = "persistent")]
    pub durability: Durability,

    /// Storage key (symbols only)
    #[arg(long = "key", required_unless_present = "key_xdr")]
    pub key: Option<Vec<String>>,

    /// Storage key (base64-encoded XDR)
    #[arg(long = "key-xdr", required_unless_present = "key")]
    pub key_xdr: Option<Vec<String>>,
}


#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Run(#[from] super::args::Error),
    #[error(transparent)]
    Locator(#[from] locator::Error),
    #[error(transparent)]
    Spec(#[from] soroban_spec_tools::Error),
    #[error(transparent)]
    StellarXdr(#[from] stellar_xdr::curr::Error),
}

impl Cmd {
    pub async fn run(&self) -> Result<(), Error> {
        let mut ledger_keys = vec![];
        self.insert_keys(&mut ledger_keys)?;
        Ok(self.args.run(ledger_keys).await?)
    }

    fn insert_keys(&self, ledger_keys: &mut Vec<LedgerKey>) -> Result<(), Error> {
        let network = self.args.network()?;
        let contract_id = self
            .contract
            .resolve_contract_id(&self.args.locator, &network.network_passphrase)?;
        let contract_address_arg = ScAddress::Contract(ContractId(Hash(contract_id.0)));

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
