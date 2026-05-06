use serde::{Deserialize, Serialize};
use std::str::FromStr;

use sep5::SeedPhrase;
use stellar_strkey::ed25519::{PrivateKey, PublicKey};

use crate::{
    print::Print,
    signer::{self, ledger, secure_store, LocalKey, SecureStoreEntry, Signer, SignerKind},
    utils,
};

use super::key::Key;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Secret(#[from] stellar_strkey::DecodeError),
    #[error(transparent)]
    SeedPhrase(#[from] sep5::error::Error),
    #[error(transparent)]
    Ed25519(#[from] ed25519_dalek::SignatureError),
    #[error("cannot parse secret (S) or seed phrase (12 or 24 word)")]
    InvalidSecretOrSeedPhrase,
    #[error(transparent)]
    Signer(#[from] signer::Error),
    #[error("Ledger does not reveal secret key")]
    LedgerDoesNotRevealSecretKey,
    #[error(transparent)]
    SecureStore(#[from] secure_store::Error),
    #[error("Secure Store does not reveal secret key")]
    SecureStoreDoesNotRevealSecretKey,
    #[error(transparent)]
    Ledger(#[from] signer::ledger::Error),
    #[error(
        "--hd-path {requested} does not match the path stored on this Ledger identity ({cached})"
    )]
    LedgerHdPathMismatch { cached: usize, requested: usize },
    #[error("--hd-path {0} is out of range for a Ledger account index")]
    HdPathOutOfRange(usize),
}

#[derive(Debug, clap::Args, Clone)]
#[group(skip)]
pub struct Args {
    /// ⚠️ Deprecated, use `--secure-store`. Enter secret (S) key when prompted
    #[arg(long)]
    pub secret_key: bool,

    /// ⚠️ Deprecated, use `--secure-store`. Enter key using 12-24 word seed phrase
    #[arg(long)]
    pub seed_phrase: bool,

    /// Save the new key in your OS's credential secure store.
    ///
    /// On Mac this uses Keychain, on Windows it is Secure Store Service, and on *nix platforms it uses a combination of the kernel keyutils and DBus-based Secret Service.
    ///
    /// This only supports seed phrases for now.
    #[arg(long)]
    pub secure_store: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum Secret {
    SecretKey {
        secret_key: String,
    },
    SeedPhrase {
        seed_phrase: String,
        // Persisted derivation index. Lets `--hd-path` set on `keys generate` /
        // `keys add` travel with the identity, so later commands derive the
        // intended account without re-passing the flag. Optional for backwards
        // compatibility with files written before this field existed.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        hd_path: Option<usize>,
    },
    // Hardware-wallet identity. The required `hardware` field tags the device
    // kind (currently only `ledger`) and disambiguates this variant under
    // `untagged`; future wallets can introduce new `HardwareKind` values
    // without a new Secret variant. The cached `public_key` lets address and
    // hint lookups succeed without the device being connected.
    Ledger {
        hardware: HardwareKind,
        public_key: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        hd_path: Option<usize>,
    },
    SecureStore {
        entry_name: String,
        // Cached public key derived from the secure-store entry. Lets us answer
        // address/hint queries without unlocking the keychain. Optional for
        // backwards compatibility with files written before this field existed.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        public_key: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        hd_path: Option<usize>,
    },
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum HardwareKind {
    Ledger,
}

impl FromStr for Secret {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if PrivateKey::from_string(s).is_ok() {
            Ok(Secret::SecretKey {
                secret_key: s.to_string(),
            })
        } else if sep5::SeedPhrase::from_str(s).is_ok() {
            Ok(Secret::SeedPhrase {
                seed_phrase: s.to_string(),
                hd_path: None,
            })
        } else if s.starts_with(secure_store::ENTRY_PREFIX) {
            Ok(Secret::SecureStore {
                entry_name: s.to_string(),
                public_key: None,
                hd_path: None,
            })
        } else {
            Err(Error::InvalidSecretOrSeedPhrase)
        }
    }
}

impl From<PrivateKey> for Secret {
    fn from(value: PrivateKey) -> Self {
        Secret::SecretKey {
            secret_key: value.to_string(),
        }
    }
}

impl From<Secret> for Key {
    fn from(value: Secret) -> Self {
        Key::Secret(value)
    }
}

impl From<SeedPhrase> for Secret {
    fn from(value: SeedPhrase) -> Self {
        Secret::SeedPhrase {
            seed_phrase: value.seed_phrase.into_phrase(),
            hd_path: None,
        }
    }
}

impl Secret {
    pub fn private_key(&self, index: Option<usize>) -> Result<PrivateKey, Error> {
        Ok(match self {
            Secret::SecretKey { secret_key } => PrivateKey::from_string(secret_key)?,
            Secret::SeedPhrase {
                seed_phrase,
                hd_path,
            } => PrivateKey::from_payload(
                &sep5::SeedPhrase::from_str(seed_phrase)?
                    .from_path_index(index.or(*hd_path).unwrap_or_default(), None)?
                    .private()
                    .0,
            )?,
            Secret::Ledger { .. } => return Err(Error::LedgerDoesNotRevealSecretKey),
            Secret::SecureStore { .. } => {
                return Err(Error::SecureStoreDoesNotRevealSecretKey);
            }
        })
    }

    pub fn public_key(&self, index: Option<usize>) -> Result<PublicKey, Error> {
        match self {
            Secret::SecureStore {
                entry_name,
                public_key,
                hd_path,
            } => {
                let effective = index.or(*hd_path);
                if let Some(cached) = cached_public_key(public_key.as_deref(), *hd_path, effective)
                {
                    return Ok(cached);
                }
                Ok(secure_store::get_public_key(entry_name, effective)?)
            }
            Secret::Ledger {
                public_key,
                hd_path: cached_hd_path,
                ..
            } => {
                let cached = cached_hd_path.unwrap_or_default();
                let requested = index.unwrap_or(cached);
                if cached != requested {
                    return Err(Error::LedgerHdPathMismatch { cached, requested });
                }
                Ok(PublicKey::from_string(public_key)?)
            }
            _ => {
                let key = self.key_pair(index)?;
                Ok(stellar_strkey::ed25519::PublicKey::from_payload(
                    key.verifying_key().as_bytes(),
                )?)
            }
        }
    }

    pub async fn signer(&self, hd_path: Option<usize>, print: Print) -> Result<Signer, Error> {
        let kind = match self {
            Secret::SecretKey { .. } | Secret::SeedPhrase { .. } => {
                let key = self.key_pair(hd_path)?;
                SignerKind::Local(LocalKey { key })
            }
            Secret::Ledger {
                hardware: HardwareKind::Ledger,
                hd_path: cached_hd_path,
                ..
            } => {
                let effective = hd_path.or(*cached_hd_path).unwrap_or_default();
                let hd_path: u32 = effective
                    .try_into()
                    .map_err(|_| Error::HdPathOutOfRange(effective))?;
                SignerKind::Ledger(ledger::new(hd_path).await?)
            }
            Secret::SecureStore {
                entry_name,
                public_key,
                hd_path: cached_hd_path,
            } => {
                let effective = hd_path.or(*cached_hd_path);
                let cached_public_key =
                    cached_public_key(public_key.as_deref(), *cached_hd_path, effective);
                SignerKind::SecureStore(SecureStoreEntry {
                    name: entry_name.clone(),
                    hd_path: effective,
                    public_key: cached_public_key,
                })
            }
        };
        Ok(Signer { kind, print })
    }

    pub fn key_pair(&self, index: Option<usize>) -> Result<ed25519_dalek::SigningKey, Error> {
        Ok(utils::into_signing_key(&self.private_key(index)?))
    }

    pub fn from_seed(seed: Option<&str>) -> Result<Self, Error> {
        Ok(seed_phrase_from_seed(seed)?.into())
    }
}

// Returns the cached public key when it can be used, or `None` to signal a
// cache miss. The cache is best-effort: a malformed cached value is ignored
// rather than propagated, and `None`/`Some(0)` are treated as the same path
// since the rest of the codebase uses `unwrap_or_default()` for hd_path.
fn cached_public_key(
    cached: Option<&str>,
    cached_hd_path: Option<usize>,
    requested_hd_path: Option<usize>,
) -> Option<PublicKey> {
    if cached_hd_path.unwrap_or_default() != requested_hd_path.unwrap_or_default() {
        return None;
    }
    PublicKey::from_string(cached?).ok()
}

pub fn seed_phrase_from_seed(seed: Option<&str>) -> Result<SeedPhrase, Error> {
    Ok(if let Some(seed) = seed.map(str::as_bytes) {
        sep5::SeedPhrase::from_entropy(seed)?
    } else {
        sep5::SeedPhrase::random(sep5::MnemonicType::Words24)?
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_PUBLIC_KEY: &str = "GAREAZZQWHOCBJS236KIE3AWYBVFLSBK7E5UW3ICI3TCRWQKT5LNLCEZ";
    const TEST_SECRET_KEY: &str = "SBF5HLRREHMS36XZNTUSKZ6FTXDZGNXOHF4EXKUL5UCWZLPBX3NGJ4BH";
    const TEST_SEED_PHRASE: &str =
        "depth decade power loud smile spatial sign movie judge february rate broccoli";

    #[test]
    fn test_from_str_for_secret_key() {
        let secret = Secret::from_str(TEST_SECRET_KEY).unwrap();
        let public_key = secret.public_key(None).unwrap();
        let private_key = secret.private_key(None).unwrap();

        assert!(matches!(secret, Secret::SecretKey { .. }));
        assert_eq!(public_key.to_string(), TEST_PUBLIC_KEY);
        assert_eq!(private_key.to_string(), TEST_SECRET_KEY);
    }

    #[test]
    fn test_secret_from_seed_phrase() {
        let secret = Secret::from_str(TEST_SEED_PHRASE).unwrap();
        let public_key = secret.public_key(None).unwrap();
        let private_key = secret.private_key(None).unwrap();

        assert!(matches!(secret, Secret::SeedPhrase { .. }));
        assert_eq!(public_key.to_string(), TEST_PUBLIC_KEY);
        assert_eq!(private_key.to_string(), TEST_SECRET_KEY);
    }

    #[test]
    fn test_secret_from_secure_store() {
        //todo: add assertion for getting public key - will need to mock the keychain and add the keypair to the keychain
        let secret = Secret::from_str("secure_store:org.stellar.cli-alice").unwrap();
        assert!(matches!(secret, Secret::SecureStore { .. }));

        let private_key_result = secret.private_key(None);
        assert!(private_key_result.is_err());
        assert!(matches!(
            private_key_result.unwrap_err(),
            Error::SecureStoreDoesNotRevealSecretKey
        ));
    }

    #[test]
    fn test_secret_from_invalid_string() {
        let secret = Secret::from_str("invalid");
        assert!(secret.is_err());
    }

    #[test]
    fn test_secure_store_toml_round_trip_with_cache() {
        let secret = Secret::SecureStore {
            entry_name: "secure_store:org.stellar.cli-alice".to_string(),
            public_key: Some(TEST_PUBLIC_KEY.to_string()),
            hd_path: None,
        };
        let serialized = toml::to_string(&secret).unwrap();
        assert!(
            serialized.contains("entry_name"),
            "expected entry_name field in TOML, got: {serialized}"
        );
        assert!(
            serialized.contains("public_key"),
            "expected public_key field in TOML, got: {serialized}"
        );
        let parsed: Secret = toml::from_str(&serialized).unwrap();
        assert_eq!(secret, parsed);
    }

    #[test]
    fn test_secure_store_legacy_toml_parses_with_none_cache() {
        // Identity files written before this feature only contain entry_name.
        // They must still parse, with public_key/hd_path defaulting to None.
        let toml_str = "entry_name = \"secure_store:org.stellar.cli-alice\"\n";
        let secret: Secret = toml::from_str(toml_str).unwrap();
        match secret {
            Secret::SecureStore {
                entry_name,
                public_key,
                hd_path,
            } => {
                assert_eq!(entry_name, "secure_store:org.stellar.cli-alice");
                assert_eq!(public_key, None);
                assert_eq!(hd_path, None);
            }
            other => panic!("expected SecureStore variant, got {other:?}"),
        }
    }

    #[test]
    fn test_secure_store_public_key_uses_cache_without_keychain_access() {
        // A non-existent entry_name guarantees a keychain lookup would fail.
        // The cached public_key should be returned without touching the keychain.
        let secret = Secret::SecureStore {
            entry_name: "secure_store:org.stellar.cli-no-such-entry".to_string(),
            public_key: Some(TEST_PUBLIC_KEY.to_string()),
            hd_path: None,
        };
        let pk = secret.public_key(None).unwrap();
        assert_eq!(pk.to_string(), TEST_PUBLIC_KEY);
    }

    #[test]
    fn test_secure_store_public_key_falls_back_to_persisted_hd_path() {
        // Bogus entry_name guarantees a keychain lookup would fail. The cache is
        // populated at the persisted hd_path; calling public_key(None) must fall
        // back to that hd_path and hit the cache rather than re-deriving at index 0.
        let secret = Secret::SecureStore {
            entry_name: "secure_store:org.stellar.cli-no-such-entry".to_string(),
            public_key: Some(TEST_PUBLIC_KEY.to_string()),
            hd_path: Some(5),
        };
        let pk = secret.public_key(None).unwrap();
        assert_eq!(pk.to_string(), TEST_PUBLIC_KEY);
    }

    #[test]
    fn test_cached_public_key_treats_none_and_zero_as_equal() {
        // `unwrap_or_default()` is used everywhere else for hd_path, so the
        // cache must treat None and Some(0) as the same path.
        assert!(cached_public_key(Some(TEST_PUBLIC_KEY), None, Some(0)).is_some());
        assert!(cached_public_key(Some(TEST_PUBLIC_KEY), Some(0), None).is_some());
        assert!(cached_public_key(Some(TEST_PUBLIC_KEY), None, None).is_some());
        assert!(cached_public_key(Some(TEST_PUBLIC_KEY), Some(0), Some(0)).is_some());

        // Different paths must still miss.
        assert!(cached_public_key(Some(TEST_PUBLIC_KEY), None, Some(1)).is_none());
        assert!(cached_public_key(Some(TEST_PUBLIC_KEY), Some(1), None).is_none());
    }

    #[test]
    fn test_cached_public_key_treats_corrupt_value_as_miss() {
        // A malformed cached value must be ignored so callers fall through to
        // the keychain instead of erroring out.
        assert!(cached_public_key(Some("not-a-public-key"), None, None).is_none());
        assert!(cached_public_key(Some(""), None, None).is_none());
    }

    #[test]
    fn test_seed_phrase_toml_round_trip_with_hd_path() {
        let secret = Secret::SeedPhrase {
            seed_phrase: TEST_SEED_PHRASE.to_string(),
            hd_path: Some(5),
        };
        let serialized = toml::to_string(&secret).unwrap();
        assert!(
            serialized.contains("hd_path"),
            "expected hd_path field in TOML, got: {serialized}"
        );
        let parsed: Secret = toml::from_str(&serialized).unwrap();
        assert_eq!(secret, parsed);
    }

    #[test]
    fn test_seed_phrase_legacy_toml_parses_with_none_hd_path() {
        // Identity files written before this feature only contain seed_phrase.
        // They must still parse, with hd_path defaulting to None.
        let toml_str = format!("seed_phrase = \"{TEST_SEED_PHRASE}\"\n");
        let secret: Secret = toml::from_str(&toml_str).unwrap();
        match secret {
            Secret::SeedPhrase {
                seed_phrase,
                hd_path,
            } => {
                assert_eq!(seed_phrase, TEST_SEED_PHRASE);
                assert_eq!(hd_path, None);
            }
            other => panic!("expected SeedPhrase variant, got {other:?}"),
        }
    }

    #[test]
    fn test_seed_phrase_uses_persisted_hd_path_when_caller_passes_none() {
        // When the caller passes None, the persisted hd_path should drive derivation.
        let secret = Secret::SeedPhrase {
            seed_phrase: TEST_SEED_PHRASE.to_string(),
            hd_path: Some(1),
        };
        let pk_at_0 = secret.public_key(Some(0)).unwrap();
        let pk_default = secret.public_key(None).unwrap();
        assert_ne!(pk_at_0.to_string(), pk_default.to_string());
    }

    #[test]
    fn test_seed_phrase_caller_hd_path_overrides_persisted() {
        // Caller's explicit hd_path argument always wins over the persisted value.
        let secret = Secret::SeedPhrase {
            seed_phrase: TEST_SEED_PHRASE.to_string(),
            hd_path: Some(1),
        };
        let pk = secret.public_key(Some(0)).unwrap();
        let sk = secret.private_key(Some(0)).unwrap();
        assert_eq!(pk.to_string(), TEST_PUBLIC_KEY);
        assert_eq!(sk.to_string(), TEST_SECRET_KEY);
    }

    #[test]
    fn test_ledger_toml_round_trip_with_hd_path() {
        let secret = Secret::Ledger {
            hardware: HardwareKind::Ledger,
            public_key: TEST_PUBLIC_KEY.to_string(),
            hd_path: Some(5),
        };
        let serialized = toml::to_string(&secret).unwrap();
        assert!(
            serialized.contains("hardware = \"ledger\""),
            "expected `hardware = \"ledger\"` tag in TOML, got: {serialized}"
        );
        assert!(
            serialized.contains("public_key"),
            "expected public_key field in TOML, got: {serialized}"
        );
        assert!(
            serialized.contains("hd_path"),
            "expected hd_path field in TOML, got: {serialized}"
        );
        let parsed: Secret = toml::from_str(&serialized).unwrap();
        assert_eq!(secret, parsed);
    }

    #[test]
    fn test_ledger_toml_omits_hd_path_when_none() {
        let secret = Secret::Ledger {
            hardware: HardwareKind::Ledger,
            public_key: TEST_PUBLIC_KEY.to_string(),
            hd_path: None,
        };
        let serialized = toml::to_string(&secret).unwrap();
        assert!(
            !serialized.contains("hd_path"),
            "expected no hd_path field in TOML when None, got: {serialized}"
        );
        let parsed: Secret = toml::from_str(&serialized).unwrap();
        assert_eq!(secret, parsed);
    }

    #[test]
    fn test_ledger_public_key_returns_cached_without_device() {
        // No emulator/device available in this test; the cached public_key
        // must be returned directly without attempting to query the device.
        let secret = Secret::Ledger {
            hardware: HardwareKind::Ledger,
            public_key: TEST_PUBLIC_KEY.to_string(),
            hd_path: Some(5),
        };
        let pk = secret.public_key(None).unwrap();
        assert_eq!(pk.to_string(), TEST_PUBLIC_KEY);
    }

    #[test]
    fn test_ledger_public_key_rejects_mismatched_hd_path() {
        // Caller asks for a different account index than the one cached on
        // disk; returning the cached key would leak the wrong address.
        let secret = Secret::Ledger {
            hardware: HardwareKind::Ledger,
            public_key: TEST_PUBLIC_KEY.to_string(),
            hd_path: Some(5),
        };
        assert!(matches!(
            secret.public_key(Some(7)).unwrap_err(),
            Error::LedgerHdPathMismatch {
                cached: 5,
                requested: 7
            },
        ));
    }

    #[test]
    fn test_ledger_public_key_treats_none_and_zero_as_equivalent() {
        let secret = Secret::Ledger {
            hardware: HardwareKind::Ledger,
            public_key: TEST_PUBLIC_KEY.to_string(),
            hd_path: None,
        };
        assert_eq!(
            secret.public_key(Some(0)).unwrap().to_string(),
            TEST_PUBLIC_KEY
        );
    }

    #[test]
    fn test_ledger_private_key_is_rejected() {
        let secret = Secret::Ledger {
            hardware: HardwareKind::Ledger,
            public_key: TEST_PUBLIC_KEY.to_string(),
            hd_path: None,
        };
        assert!(matches!(
            secret.private_key(None).unwrap_err(),
            Error::LedgerDoesNotRevealSecretKey,
        ));
    }

    #[test]
    fn test_ledger_toml_does_not_collide_with_secure_store() {
        // SecureStore TOMLs (entry_name + optional cached public_key) must not
        // be mis-deserialized as Ledger now that Ledger also carries public_key.
        let toml_str = "entry_name = \"secure_store:org.stellar.cli-alice\"\n\
                        public_key = \"GAREAZZQWHOCBJS236KIE3AWYBVFLSBK7E5UW3ICI3TCRWQKT5LNLCEZ\"\n";
        let secret: Secret = toml::from_str(toml_str).unwrap();
        assert!(matches!(secret, Secret::SecureStore { .. }));
    }
}
