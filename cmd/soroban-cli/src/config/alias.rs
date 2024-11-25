use std::{collections::HashMap, convert::Infallible, str::FromStr};

use serde::{Deserialize, Serialize};

use super::locator;

#[derive(Serialize, Deserialize, Default)]
pub struct Data {
    pub ids: HashMap<String, String>,
}

/// Address can be either a contract address, C.. or eventually an alias of a contract address.
#[derive(Clone, Debug)]
pub enum ContractAddress {
    ContractId(stellar_strkey::Contract),
    Alias(String),
}

impl Default for ContractAddress {
    fn default() -> Self {
        ContractAddress::Alias(String::default())
    }
}

impl FromStr for ContractAddress {
    type Err = Infallible;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(stellar_strkey::Contract::from_str(value).map_or_else(
            |_| ContractAddress::Alias(value.to_string()),
            ContractAddress::ContractId,
        ))
    }
}

impl ContractAddress {
    pub fn resolve_contract_id(
        &self,
        locator: &locator::Args,
        network_passphrase: &str,
    ) -> Result<stellar_strkey::Contract, locator::Error> {
        match self {
            ContractAddress::ContractId(muxed_account) => Ok(*muxed_account),
            ContractAddress::Alias(alias) => locator
                .get_contract_id(alias, network_passphrase)?
                .ok_or_else(|| locator::Error::ContractNotFound(alias.to_owned())),
        }
    }
}
