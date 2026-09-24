use std::str::FromStr;

use crate::xdr;

use super::{alias, key, locator, UnresolvedContract};

/// `ScAddress` can be either a resolved `xdr::ScAddress` or an alias of a `Contract` or `MuxedAccount`.
#[allow(clippy::module_name_repetitions)]
#[derive(Clone, Debug)]
pub enum UnresolvedScAddress {
    Resolved(xdr::ScAddress),
    Alias(String),
}

impl Default for UnresolvedScAddress {
    fn default() -> Self {
        UnresolvedScAddress::Alias(String::default())
    }
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Locator(#[from] locator::Error),
    #[error(transparent)]
    Key(#[from] key::Error),
    #[error("Account alias \"{0}\" not Found")]
    AccountAliasNotFound(String),
    #[error("alias '{0}' is reserved for the native asset contract but also matches a stored key; pass an explicit contract (C...) or account (G...) address instead")]
    ReservedAliasShadowsKey(String),
}

impl FromStr for UnresolvedScAddress {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(xdr::ScAddress::from_str(value).map_or_else(
            |_| UnresolvedScAddress::Alias(value.to_string()),
            UnresolvedScAddress::Resolved,
        ))
    }
}

impl UnresolvedScAddress {
    pub fn resolve(
        self,
        locator: &locator::Args,
        network_passphrase: &str,
        hd_path: Option<u32>,
    ) -> Result<xdr::ScAddress, Error> {
        let alias = match self {
            UnresolvedScAddress::Resolved(addr) => return Ok(addr),
            UnresolvedScAddress::Alias(alias) => alias,
        };
        let contract = UnresolvedContract::resolve_alias(&alias, locator, network_passphrase);
        let key = locator.read_key(&alias);
        match (contract, key) {
            (Ok(contract), Ok(_)) => {
                // A reserved built-in alias (e.g. `native`) shadows an on-disk
                // key of the same name. Preferring either side could send funds
                // to the wrong address, so refuse and ask for an explicit one.
                if alias::is_reserved(&alias) {
                    return Err(Error::ReservedAliasShadowsKey(alias));
                }
                eprintln!(
                    "Warning: ScAddress alias {alias} is ambiguous, assuming it is a contract"
                );
                Ok(xdr::ScAddress::Contract(stellar_xdr::ContractId(
                    xdr::Hash(contract.0),
                )))
            }
            (Ok(contract), _) => Ok(xdr::ScAddress::Contract(stellar_xdr::ContractId(
                xdr::Hash(contract.0),
            ))),
            // Surface a shadowed reserved-alias collision rather than masking it
            // with a generic "not found" error. This must precede the key arm:
            // when both a stored `native` alias and a `native` key exist, the
            // collision has to win so resolution can't silently pick the key.
            (Err(err @ locator::Error::ShadowedReservedAlias { .. }), _) => Err(err.into()),
            (_, Ok(key)) => Ok(xdr::ScAddress::Account(
                key.muxed_account(hd_path)?.account_id(),
            )),
            _ => Err(Error::AccountAliasNotFound(alias)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::key::Key;
    use crate::config::locator::KeyType;
    use std::str::FromStr;

    #[test]
    fn resolve_errors_when_reserved_alias_shadows_key() {
        let dir = tempfile::tempdir().unwrap();
        let locator = locator::Args {
            config_dir: Some(dir.path().to_path_buf()),
        };
        let network_passphrase = "Test Network";
        let native = alias::NATIVE;

        // A key named after the native alias, created before it became
        // reserved. Written directly since `write_identity` now rejects it.
        let key =
            Key::from_str("SBEQMTXGCLDFQG3OXMRSMGLKJCPROAHB5GZCCGVZERDI645LCCCRLFGY").unwrap();
        KeyType::Identity.write(native, &key, dir.path()).unwrap();

        let err = UnresolvedScAddress::Alias(native.to_string())
            .resolve(&locator, network_passphrase, None)
            .unwrap_err();

        assert!(matches!(err, Error::ReservedAliasShadowsKey(alias) if alias == native));
    }

    #[test]
    fn resolve_errors_when_reserved_alias_shadowed_by_stored_alias_and_key() {
        let dir = tempfile::tempdir().unwrap();
        let locator = locator::Args {
            config_dir: Some(dir.path().to_path_buf()),
        };
        let network_passphrase = "Test Network";
        let native = alias::NATIVE;

        // Both pre-upgrade artifacts exist: a key named after the native alias
        // and a stored alias of the same name pointing at another contract. The
        // shadowed-alias error must win rather than the resolution silently
        // picking the key.
        let key =
            Key::from_str("SBEQMTXGCLDFQG3OXMRSMGLKJCPROAHB5GZCCGVZERDI645LCCCRLFGY").unwrap();
        KeyType::Identity.write(native, &key, dir.path()).unwrap();

        let contract_ids = dir.path().join("contract-ids");
        std::fs::create_dir_all(&contract_ids).unwrap();
        std::fs::write(
            contract_ids.join(format!("{native}.json")),
            format!(
                r#"{{"ids":{{"{network_passphrase}":"CA3D5KRYM6CB7OWQ6TWYRR3Z4T7GNZLKERYNZGGA5SOAOPIFY6YQGAXE"}}}}"#
            ),
        )
        .unwrap();

        let err = UnresolvedScAddress::Alias(native.to_string())
            .resolve(&locator, network_passphrase, None)
            .unwrap_err();

        assert!(matches!(
            err,
            Error::Locator(locator::Error::ShadowedReservedAlias { alias, .. }) if alias == native
        ));
    }
}
