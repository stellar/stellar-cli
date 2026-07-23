use std::str::FromStr;

use crate::{
    config::{alias::UnresolvedContract, locator},
    tx::builder,
    utils::contract_id_hash_from_asset,
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Locator(#[from] locator::Error),
    #[error(transparent)]
    Asset(#[from] builder::asset::Error),
}

impl Error {
    /// Machine-readable discriminator for the JSON error envelope's `type` field.
    #[must_use]
    pub fn error_type(&self) -> &'static str {
        match self {
            Error::Locator(_) => "config",
            Error::Asset(_) => "invalid_asset",
        }
    }
}

/// What a token reference resolved to.
#[derive(Clone, Debug)]
pub enum TokenKind {
    /// A Stellar Asset Contract (SAC) wrapping this classic asset.
    Sac(crate::xdr::Asset),
    /// A plain SEP-41 contract addressed by id or alias.
    Contract,
}

/// A token reference resolved to a concrete contract id, together with what it
/// turned out to be. This is the single answer to both "what contract id?" and
/// "is this a SAC, and for which asset?", so SAC-awareness (deploy hints,
/// `sac_not_deployed` vs `contract_not_found` errors) stays consistent
/// everywhere.
#[derive(Clone, Debug)]
pub struct ResolvedToken {
    pub contract_id: stellar_strkey::Contract,
    pub kind: TokenKind,
}

impl ResolvedToken {
    /// Returns `true` if this token is a Stellar Asset Contract.
    #[must_use]
    pub fn is_sac(&self) -> bool {
        matches!(self.kind, TokenKind::Sac(_))
    }

    /// The classic asset backing this token, or `None` for a plain contract.
    #[must_use]
    pub fn asset(&self) -> Option<&crate::xdr::Asset> {
        match &self.kind {
            TokenKind::Sac(asset) => Some(asset),
            TokenKind::Contract => None,
        }
    }
}

/// An unresolved token reference parsed from a user-supplied string.
///
/// The shape of the value decides how it is interpreted:
/// * `native` or `CODE:ISSUER` → a Stellar Asset Contract (SAC), resolved from
///   the classic asset.
/// * anything else → a contract, either a `C…` strkey or a saved alias.
#[derive(Clone, Debug)]
pub enum UnresolvedToken {
    /// A Stellar Asset Contract addressed by its underlying classic asset.
    Asset(builder::Asset),
    /// A SEP-41 contract addressed directly by id or alias.
    Contract(UnresolvedContract),
}

impl FromStr for UnresolvedToken {
    type Err = builder::asset::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        // `native` and the `CODE:ISSUER` shape are both classic assets, resolved
        // to their Stellar Asset Contract — so a missing SAC reports
        // `sac_not_deployed`, not `contract_not_found`. Everything else is a
        // contract id or a saved alias.
        if value == "native" || value.contains(':') {
            Ok(UnresolvedToken::Asset(value.parse()?))
        } else {
            // `UnresolvedContract::from_str` is infallible.
            Ok(UnresolvedToken::Contract(value.parse().unwrap()))
        }
    }
}

impl UnresolvedToken {
    /// Resolve this reference to a concrete contract id and classify it as a SAC
    /// (with its asset) or a plain contract.
    pub fn resolve(
        &self,
        locator: &locator::Args,
        network_passphrase: &str,
    ) -> Result<ResolvedToken, Error> {
        match self {
            UnresolvedToken::Asset(asset) => {
                let asset = asset.resolve(locator)?;
                let contract_id = contract_id_hash_from_asset(&asset, network_passphrase);
                Ok(ResolvedToken {
                    contract_id,
                    kind: TokenKind::Sac(asset),
                })
            }
            UnresolvedToken::Contract(contract) => {
                let contract_id = contract.resolve_contract_id(locator, network_passphrase)?;
                Ok(ResolvedToken {
                    contract_id,
                    kind: TokenKind::Contract,
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A valid contract strkey borrowed from the CLI's own help text.
    const CONTRACT: &str = "CCR6QKTWZQYW6YUJ7UP7XXZRLWQPFRV6SWBLQS4ZQOSAF4BOUD77OTE2";
    const NETWORK: &str = "Test Network";

    #[test]
    fn native_parses_as_asset() {
        assert!(matches!(
            "native".parse::<UnresolvedToken>().unwrap(),
            UnresolvedToken::Asset(builder::Asset::Native)
        ));
    }

    #[test]
    fn code_issuer_parses_as_asset() {
        assert!(matches!(
            "USDC:issuer".parse::<UnresolvedToken>().unwrap(),
            UnresolvedToken::Asset(builder::Asset::Asset(..))
        ));
    }

    #[test]
    fn contract_strkey_parses_as_resolved_contract() {
        assert!(matches!(
            CONTRACT.parse::<UnresolvedToken>().unwrap(),
            UnresolvedToken::Contract(UnresolvedContract::Resolved(_))
        ));
    }

    #[test]
    fn bare_name_parses_as_contract_alias() {
        assert!(matches!(
            "alice".parse::<UnresolvedToken>().unwrap(),
            UnresolvedToken::Contract(UnresolvedContract::Alias(_))
        ));
    }

    #[test]
    fn native_resolves_to_sac() {
        let locator = locator::Args::default();
        let resolved = "native"
            .parse::<UnresolvedToken>()
            .unwrap()
            .resolve(&locator, NETWORK)
            .unwrap();

        assert!(resolved.is_sac());
        assert!(matches!(resolved.asset(), Some(crate::xdr::Asset::Native)));
        assert_eq!(
            resolved.contract_id,
            contract_id_hash_from_asset(&crate::xdr::Asset::Native, NETWORK)
        );
    }

    #[test]
    fn code_issuer_resolves_to_sac() {
        let locator = locator::Args::default();
        // A `CODE:ISSUER` reference with a concrete G… issuer needs no alias
        // lookup, so it resolves without a populated locator.
        let resolved = "USDC:GBBD47IF6LWK7P7MDEVSCWR7DPUWV3NY3DTQEVFL4NAT4AQH3ZLLFLA5"
            .parse::<UnresolvedToken>()
            .unwrap()
            .resolve(&locator, NETWORK)
            .unwrap();

        assert!(resolved.is_sac());
        assert!(matches!(
            resolved.asset(),
            Some(crate::xdr::Asset::CreditAlphanum4(_))
        ));
    }

    #[test]
    fn contract_strkey_resolves_to_contract() {
        let locator = locator::Args::default();
        let resolved = CONTRACT
            .parse::<UnresolvedToken>()
            .unwrap()
            .resolve(&locator, NETWORK)
            .unwrap();

        assert!(!resolved.is_sac());
        assert!(resolved.asset().is_none());
        assert_eq!(resolved.contract_id, CONTRACT.parse().unwrap());
    }
}
