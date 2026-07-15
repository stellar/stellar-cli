use std::{collections::HashMap, convert::Infallible, str::FromStr};

use serde::{Deserialize, Serialize};
use stellar_strkey::Contract;

use super::locator;
use crate::utils::contract_id_hash_from_asset;
use crate::xdr::Asset;

#[derive(Serialize, Deserialize, Default)]
pub struct Data {
    pub ids: HashMap<String, String>,
}

/// The reserved, built-in contract alias. It resolves to the native asset (XLM)
/// Stellar Asset Contract for the current network and cannot be created,
/// overwritten, or removed by users.
pub const NATIVE: &str = "native";

/// Returns `true` if `alias` is the reserved, built-in alias that users cannot
/// create, overwrite, or remove.
#[must_use]
pub fn is_reserved(alias: &str) -> bool {
    alias == NATIVE
}

/// Resolves the reserved, built-in alias to its contract for `network_passphrase`,
/// or returns `None` if `alias` is not reserved. This is the single source of
/// truth for what the reserved alias points to, so resolution stays consistent
/// across `get_contract_id`, `alias show`, and `alias ls`.
#[must_use]
pub fn resolve_reserved(alias: &str, network_passphrase: &str) -> Option<Contract> {
    if is_reserved(alias) {
        Some(contract_id_hash_from_asset(
            &Asset::Native,
            network_passphrase,
        ))
    } else {
        None
    }
}

/// Errors if `alias` is a reserved, built-in alias. Call this before doing any
/// work (building, simulating, deploying, or writing config) so that a reserved
/// alias fails fast.
pub fn validate_reserved_aliases(alias: &str) -> Result<(), locator::Error> {
    if is_reserved(alias) {
        return Err(locator::Error::ContractAliasReserved(alias.to_owned()));
    }
    Ok(())
}

/// Address can be either a contract address, C.. or eventually an alias of a contract address.
#[derive(Clone, Debug)]
pub enum UnresolvedContract {
    Resolved(stellar_strkey::Contract),
    Alias(String),
}

impl Default for UnresolvedContract {
    fn default() -> Self {
        UnresolvedContract::Alias(String::default())
    }
}

impl FromStr for UnresolvedContract {
    type Err = Infallible;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(stellar_strkey::Contract::from_str(value).map_or_else(
            |_| UnresolvedContract::Alias(value.to_string()),
            UnresolvedContract::Resolved,
        ))
    }
}

impl UnresolvedContract {
    pub fn resolve_contract_id(
        &self,
        locator: &locator::Args,
        network_passphrase: &str,
    ) -> Result<stellar_strkey::Contract, locator::Error> {
        match self {
            UnresolvedContract::Resolved(contract) => Ok(*contract),
            UnresolvedContract::Alias(alias) => {
                Self::resolve_alias(alias, locator, network_passphrase)
            }
        }
    }

    pub fn resolve_alias(
        alias: &str,
        locator: &locator::Args,
        network_passphrase: &str,
    ) -> Result<stellar_strkey::Contract, locator::Error> {
        locator
            .get_contract_id(alias, network_passphrase)?
            .ok_or_else(|| locator::Error::ContractNotFound(alias.to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_is_reserved() {
        assert!(is_reserved("native"));
        assert!(validate_reserved_aliases("native").is_err());
    }

    #[test]
    fn regular_aliases_are_not_reserved() {
        assert!(!is_reserved("my-token"));
        assert!(validate_reserved_aliases("my-token").is_ok());
    }
}
