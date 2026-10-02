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
    /// Whether this resolves to a muxed (`M…`) identity — either a literal `M…`
    /// strkey or an alias whose stored key is muxed. Contexts that a muxed
    /// address can't be used in — e.g. an allowance `spender`, or an `owner` the
    /// host rejects — call this to fail up front with a clear message instead of
    /// an opaque host error mid-simulation.
    #[must_use]
    pub fn is_muxed(&self, locator: &locator::Args, network_passphrase: &str) -> bool {
        let alias = match self {
            // A literal `M…` is already resolved; catch it here so a direct
            // strkey is rejected as readily as an alias.
            UnresolvedScAddress::Resolved(addr) => {
                return matches!(addr, xdr::ScAddress::MuxedAccount(_));
            }
            UnresolvedScAddress::Alias(alias) => alias,
        };
        // Mirror `resolve`'s precedence: a contract alias wins when both a
        // contract alias and a stored key exist, so the muxed key is never
        // picked. A shadowed reserved-alias collision is likewise surfaced by
        // `resolve` itself, so don't mask it as a muxed rejection. Only a muxed
        // key that would actually be resolved matters.
        match UnresolvedContract::resolve_alias(alias, locator, network_passphrase) {
            Ok(_) | Err(locator::Error::ShadowedReservedAlias { .. }) => return false,
            Err(_) => {}
        }
        matches!(locator.read_key(alias), Ok(key::Key::MuxedAccount(_)))
    }

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
            // Preserve a muxed (`M…`) key as a muxed `ScAddress` rather than
            // downgrading to its base `G…` account: collapsing it would target a
            // different recipient than the one named. Contexts that can't accept
            // a muxed address reject it up front (see `is_muxed`).
            (_, Ok(key)) => Ok(match key.muxed_account(hd_path)? {
                xdr::MuxedAccount::Ed25519(ed25519) => xdr::ScAddress::Account(xdr::AccountId(
                    xdr::PublicKey::PublicKeyTypeEd25519(ed25519),
                )),
                xdr::MuxedAccount::MuxedEd25519(xdr::MuxedAccountMed25519 { id, ed25519 }) => {
                    xdr::ScAddress::MuxedAccount(xdr::MuxedEd25519Account { id, ed25519 })
                }
            }),
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

    const MUXED: &str = "MA3D5KRYM6CB7OWQ6TWYRR3Z4T7GNZLKERYNZGGA5SOAOPIFY6YQGAAAAAAAAAPCICBKU";

    #[test]
    fn resolve_preserves_muxed_account_alias() {
        let dir = tempfile::tempdir().unwrap();
        let locator = locator::Args {
            config_dir: Some(dir.path().to_path_buf()),
        };
        let network_passphrase = "Test Network";

        // An alias whose stored key is muxed must resolve to a muxed
        // `ScAddress`, not silently downgrade to its base `G…` account, so a
        // transfer targets the exact recipient (mux id included) that was named.
        let key = Key::from_str(MUXED).unwrap();
        KeyType::Identity.write("bobmux", &key, dir.path()).unwrap();

        let resolved = UnresolvedScAddress::Alias("bobmux".to_string())
            .resolve(&locator, network_passphrase, None)
            .unwrap();

        assert_eq!(resolved, xdr::ScAddress::from_str(MUXED).unwrap());
        assert!(matches!(resolved, xdr::ScAddress::MuxedAccount(_)));
    }

    #[test]
    fn is_muxed_true_for_resolved_muxed_strkey() {
        let dir = tempfile::tempdir().unwrap();
        let locator = locator::Args {
            config_dir: Some(dir.path().to_path_buf()),
        };
        let network_passphrase = "Test Network";

        // A literal `M…` parses to a resolved muxed `ScAddress`; the guard must
        // catch it too, so a direct strkey is rejected as readily as an alias.
        let address = UnresolvedScAddress::from_str(MUXED).unwrap();
        assert!(matches!(address, UnresolvedScAddress::Resolved(_)));
        assert!(address.is_muxed(&locator, network_passphrase));

        // A resolved non-muxed account is not flagged.
        let account = UnresolvedScAddress::from_str(
            "GA3D5KRYM6CB7OWQ6TWYRR3Z4T7GNZLKERYNZGGA5SOAOPIFY6YQHES5",
        )
        .unwrap();
        assert!(!account.is_muxed(&locator, network_passphrase));
    }

    #[test]
    fn is_muxed_true_for_stored_muxed_key() {
        let dir = tempfile::tempdir().unwrap();
        let locator = locator::Args {
            config_dir: Some(dir.path().to_path_buf()),
        };
        let network_passphrase = "Test Network";

        let key = Key::from_str(MUXED).unwrap();
        KeyType::Identity.write("owner", &key, dir.path()).unwrap();

        assert!(
            UnresolvedScAddress::Alias("owner".to_string()).is_muxed(&locator, network_passphrase)
        );
    }

    #[test]
    fn is_muxed_false_when_reserved_alias_is_shadowed() {
        let dir = tempfile::tempdir().unwrap();
        let locator = locator::Args {
            config_dir: Some(dir.path().to_path_buf()),
        };
        let network_passphrase = "Test Network";
        let native = alias::NATIVE;

        // A reserved alias shadowed by a stored contract id, with a muxed key of
        // the same name. `resolve` surfaces the collision error, so
        // `is_muxed` must not mask it by reporting a muxed rejection.
        let key = Key::from_str(MUXED).unwrap();
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

        assert!(
            !UnresolvedScAddress::Alias(native.to_string()).is_muxed(&locator, network_passphrase)
        );
    }

    #[test]
    fn is_muxed_false_when_contract_alias_takes_precedence() {
        let dir = tempfile::tempdir().unwrap();
        let locator = locator::Args {
            config_dir: Some(dir.path().to_path_buf()),
        };
        let network_passphrase = "Test Network";

        // A muxed key and a contract alias share the name. `resolve` picks the
        // contract (never downgrading to a `G…` account), so `is_muxed`
        // must not reject it.
        let key = Key::from_str(MUXED).unwrap();
        KeyType::Identity.write("dual", &key, dir.path()).unwrap();

        let contract_ids = dir.path().join("contract-ids");
        std::fs::create_dir_all(&contract_ids).unwrap();
        std::fs::write(
            contract_ids.join("dual.json"),
            format!(
                r#"{{"ids":{{"{network_passphrase}":"CA3D5KRYM6CB7OWQ6TWYRR3Z4T7GNZLKERYNZGGA5SOAOPIFY6YQGAXE"}}}}"#
            ),
        )
        .unwrap();

        assert!(
            !UnresolvedScAddress::Alias("dual".to_string()).is_muxed(&locator, network_passphrase)
        );
    }
}
