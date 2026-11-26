use std::{collections::HashMap, convert::Infallible, str::FromStr};

use serde::{Deserialize, Serialize};
use stellar_strkey::Contract;

use super::{locator, secret};
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
#[derive(Clone)]
pub enum UnresolvedContract {
    Resolved(stellar_strkey::Contract),
    Alias(String),
}

impl std::fmt::Debug for UnresolvedContract {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UnresolvedContract::Resolved(contract) => {
                f.debug_tuple("Resolved").field(contract).finish()
            }
            // A genuine alias is safe to show, but never echo a secret key or
            // seed phrase pasted where a contract id was expected — e.g. under
            // `--verbose`, which debug-prints parsed arguments.
            UnresolvedContract::Alias(alias) if secret::looks_like_secret(alias) => {
                f.debug_tuple("Alias").field(&"<alias or secret>").finish()
            }
            UnresolvedContract::Alias(alias) => f.debug_tuple("Alias").field(alias).finish(),
        }
    }
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
        let result = locator.get_contract_id(alias, network_passphrase);

        // A secret key or seed phrase pasted where a contract id was expected is
        // a misplaced secret, not a real alias: collapse every outcome — a miss,
        // name validation (a seed phrase's spaces fail it), or a parse/I/O error
        // keyed on the pasted value — to the payload-less concealed error. None of
        // these is actionable for a pasted secret, and propagating one would echo
        // it, since the locator error messages interpolate the raw alias (e.g.
        // `CannotParseContractId`, `FileRead`). The concealed variant stores
        // nothing, so the secret can't leak through a derived `Debug` either.
        // Non-secret input keeps full error propagation below.
        if secret::looks_like_secret(alias) {
            return result
                .ok()
                .flatten()
                .ok_or(locator::Error::ContractNotFoundConcealed);
        }

        result?.ok_or_else(|| locator::Error::ContractNotFound(alias.to_owned()))
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

    #[test]
    fn debug_conceals_secret_bearing_alias() {
        // `--verbose` debug-prints parsed arguments; a secret pasted where a
        // contract id was expected must not surface through the `Alias` variant.
        for input in [
            "SBF5HLRREHMS36XZNTUSKZ6FTXDZGNXOHF4EXKUL5UCWZLPBX3NGJ4BX",
            "illness spike retreat truth genius clock brain pass fit cave bargain xyzzy",
        ] {
            let contract = UnresolvedContract::from_str(input).unwrap();
            assert!(!format!("{contract:?}").contains(input));
        }
    }

    #[test]
    fn debug_echoes_plain_alias() {
        let contract = UnresolvedContract::from_str("my-token").unwrap();
        assert!(format!("{contract:?}").contains("my-token"));
    }

    #[test]
    fn resolve_conceals_secret_with_malformed_stored_id() {
        // A secret-shaped alias with an existing but malformed stored contract-id
        // file makes `get_contract_id` return `CannotParseContractId(alias, ..)`,
        // whose message interpolates the raw alias. Resolution must collapse that
        // to the concealing not-found error rather than propagate (and echo) it.
        let secret = "SBF5HLRREHMS36XZNTUSKZ6FTXDZGNXOHF4EXKUL5UCWZLPBX3NGJ4BX";
        let dir = tempfile::tempdir().unwrap();
        let locator = locator::Args {
            config_dir: Some(dir.path().to_path_buf()),
        };
        let network_passphrase = "Test Network";

        let contract_ids = dir.path().join("contract-ids");
        std::fs::create_dir_all(&contract_ids).unwrap();
        std::fs::write(
            contract_ids.join(format!("{secret}.json")),
            format!(r#"{{"ids":{{"{network_passphrase}":"not-a-valid-contract-id"}}}}"#),
        )
        .unwrap();

        let err =
            UnresolvedContract::resolve_alias(secret, &locator, network_passphrase).unwrap_err();
        assert!(!err.to_string().contains(secret), "leaked secret: {err}");
    }
}
