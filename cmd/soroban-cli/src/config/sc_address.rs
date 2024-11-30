use std::str::FromStr;

use crate::xdr;

use super::{address, locator, ContractAddress};

/// `ScAddress` can be either a resolved `xdr::ScAddress` or an alias of a `Contract` or `MuxedAccount`.
#[derive(Clone, Debug)]
pub enum ScAddress {
    Address(xdr::ScAddress),
    Alias(String),
}

impl Default for ScAddress {
    fn default() -> Self {
        ScAddress::Alias(String::default())
    }
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Locator(#[from] locator::Error),
    #[error(transparent)]
    Address(#[from] address::Error),
    #[error("Account alias not Found{0}")]
    AccountAliasNotFound(String),
}

impl FromStr for ScAddress {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(xdr::ScAddress::from_str(value)
            .map_or_else(|_| ScAddress::Alias(value.to_string()), ScAddress::Address))
    }
}

impl ScAddress {
    pub fn resolve(
        self,
        locator: &locator::Args,
        network_passphrase: &str,
    ) -> Result<xdr::ScAddress, Error> {
        let alias = match self {
            ScAddress::Address(addr) => return Ok(addr),
            ScAddress::Alias(alias) => alias,
        };
        let contract = ContractAddress::resolve_alias(&alias, locator, network_passphrase);
        let muxed_account = super::Address::resolve_muxed_account_with_alias(&alias, locator, None);
        match (contract, muxed_account) {
            (Ok(contract), _) => Ok(xdr::ScAddress::Contract(xdr::Hash(contract.0))),
            (_, Ok(muxed_account)) => Ok(xdr::ScAddress::Account(muxed_account.account_id())),
            _ => Err(Error::AccountAliasNotFound(alias)),
        }
    }
}
