use std::str::FromStr;

use crate::{
    config::{alias::UnresolvedContract, locator},
    output::Format,
    tx::builder,
    utils::contract_id_hash_from_asset,
};

/// Output format shared by the `stellar token` subcommands.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, clap::ValueEnum, Default)]
pub enum OutputFormat {
    /// Human-readable text.
    #[default]
    Text,
    /// Compact, single-line JSON receipt.
    Json,
    /// Formatted (multiline) JSON receipt.
    JsonFormatted,
}

impl From<OutputFormat> for Format {
    fn from(value: OutputFormat) -> Self {
        match value {
            OutputFormat::Text => Format::Readable,
            OutputFormat::Json => Format::Json,
            OutputFormat::JsonFormatted => Format::JsonFormatted,
        }
    }
}

/// A `stellar token` target, resolved from the `--id` value.
///
/// The shape of the value decides how it is interpreted:
/// * `native` or `CODE:ISSUER` → a Stellar Asset Contract (SAC), resolved from
///   the asset.
/// * anything else → a contract, either a `C…` strkey or a saved alias.
#[derive(Clone, Debug)]
pub enum TokenTarget {
    /// A SEP-41 contract addressed directly by id or alias.
    Contract(UnresolvedContract),
    /// A Stellar Asset Contract addressed by its underlying classic asset.
    Asset(builder::Asset),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Locator(#[from] locator::Error),
    #[error(transparent)]
    Asset(#[from] builder::asset::Error),
}

impl FromStr for TokenTarget {
    type Err = builder::asset::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        // `native` and `CODE:ISSUER` are the two shapes an asset can take; both
        // route to a SAC. Everything else is a contract id or alias.
        if value == "native" || value.contains(':') {
            Ok(TokenTarget::Asset(value.parse()?))
        } else {
            // `UnresolvedContract::from_str` is infallible.
            Ok(TokenTarget::Contract(value.parse().unwrap()))
        }
    }
}

impl TokenTarget {
    /// Resolve this target to a concrete contract id for the given network.
    pub fn resolve_contract_id(
        &self,
        locator: &locator::Args,
        network_passphrase: &str,
    ) -> Result<stellar_strkey::Contract, Error> {
        match self {
            TokenTarget::Contract(contract) => {
                Ok(contract.resolve_contract_id(locator, network_passphrase)?)
            }
            TokenTarget::Asset(asset) => {
                let asset = asset.resolve(locator)?;
                Ok(contract_id_hash_from_asset(&asset, network_passphrase))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A valid contract strkey borrowed from the CLI's own help text.
    const CONTRACT: &str = "CCR6QKTWZQYW6YUJ7UP7XXZRLWQPFRV6SWBLQS4ZQOSAF4BOUD77OTE2";

    #[test]
    fn native_parses_as_asset() {
        assert!(matches!(
            "native".parse::<TokenTarget>().unwrap(),
            TokenTarget::Asset(builder::Asset::Native)
        ));
    }

    #[test]
    fn code_issuer_parses_as_asset() {
        assert!(matches!(
            "USDC:issuer".parse::<TokenTarget>().unwrap(),
            TokenTarget::Asset(builder::Asset::Asset(..))
        ));
    }

    #[test]
    fn contract_strkey_parses_as_resolved_contract() {
        assert!(matches!(
            CONTRACT.parse::<TokenTarget>().unwrap(),
            TokenTarget::Contract(UnresolvedContract::Resolved(_))
        ));
    }

    #[test]
    fn bare_name_parses_as_contract_alias() {
        assert!(matches!(
            "alice".parse::<TokenTarget>().unwrap(),
            TokenTarget::Contract(UnresolvedContract::Alias(_))
        ));
    }
}
