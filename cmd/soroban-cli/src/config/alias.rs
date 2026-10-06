use std::{collections::HashMap, convert::Infallible, str::FromStr};

use serde::{Deserialize, Serialize};
use stellar_strkey::Contract;

use super::{
    arg_name::{ArgName, ArgNameParser, FromArg},
    locator,
};
use crate::config::token::UnresolvedToken;
use crate::tx::builder;

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
pub fn resolve_reserved(
    alias: &str,
    locator: &locator::Args,
    network_passphrase: &str,
) -> Option<Contract> {
    if !is_reserved(alias) {
        return None;
    }

    // The reserved alias points at the native asset's Stellar Asset Contract.
    // Route it through the shared token resolver so its id stays identical to
    // every other `native` resolution. `locator` is unused for native (there is
    // no issuer alias to look up) but is threaded through for signature parity
    // with the resolver; native therefore can never fail to resolve.
    let resolved = UnresolvedToken::Asset(builder::Asset::Native)
        .resolve(locator, network_passphrase)
        .expect("the reserved native alias always resolves to its SAC");
    Some(resolved.contract_id)
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
    Alias { alias: String, arg: ArgName },
}

impl Default for UnresolvedContract {
    fn default() -> Self {
        UnresolvedContract::from_arg("", ArgName::default())
    }
}

impl FromStr for UnresolvedContract {
    type Err = Infallible;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(UnresolvedContract::from_arg(value, ArgName::default()))
    }
}

impl FromArg for UnresolvedContract {
    fn from_arg(value: &str, arg: ArgName) -> Self {
        stellar_strkey::Contract::from_str(value).map_or_else(
            |_| UnresolvedContract::Alias {
                alias: value.to_string(),
                arg,
            },
            UnresolvedContract::Resolved,
        )
    }
}

impl clap::builder::ValueParserFactory for UnresolvedContract {
    type Parser = ArgNameParser<Self>;

    fn value_parser() -> Self::Parser {
        ArgNameParser::default()
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
            UnresolvedContract::Alias { alias, arg } => {
                Self::resolve_alias(alias, locator, network_passphrase).map_err(|e| match e {
                    // Name the argument rather than echoing the value, which
                    // may be a secret key or seed phrase pasted in the wrong
                    // place. A seed phrase fails alias name validation.
                    locator::Error::ContractNotFound(_) | locator::Error::InvalidName(_)
                        if arg.is_known() =>
                    {
                        locator::Error::ArgContractNotFound {
                            arg: arg.clone(),
                            hint: locator::wasm_hash_hint(alias),
                        }
                    }
                    e => e,
                })
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
