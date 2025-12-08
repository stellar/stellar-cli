use clap::arg;
use serde::{Deserialize, Serialize};
use std::{
    str::FromStr,
    sync::{Arc, OnceLock},
};

use sep5::SeedPhrase;
use stellar_strkey::ed25519::{PrivateKey, PublicKey};

use crate::{
    print::Print,
    signer::{
        self, ledger,
        secure_store_entry::{self, SecureStoreEntry},
        LocalKey, Signer, SignerKind,
    },
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
    SecureStore(#[from] secure_store_entry::Error),
    #[error("Secure Store does not reveal secret key")]
    SecureStoreDoesNotRevealSecretKey,
    #[error(transparent)]
    Ledger(#[from] signer::ledger::Error),
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

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Secret {
    SecretKey {
        secret_key: String,
    },
    SeedPhrase {
        seed_phrase: String,
    },
    Ledger,
    SecureStore {
        entry_name: String,
        #[serde(skip)]
        #[serde(default)]
        cached_entry: Arc<OnceLock<SecureStoreEntry>>,
    },
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
            })
        } else if s == "ledger" {
            Ok(Secret::Ledger)
        } else if s.starts_with(secure_store_entry::ENTRY_PREFIX) {
            Ok(Secret::SecureStore {
                entry_name: s.to_string(),
                cached_entry: OnceLock::new().into(),
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
        }
    }
}

impl Secret {
    pub fn private_key(&self, index: Option<usize>) -> Result<PrivateKey, Error> {
        Ok(match self {
            Secret::SecretKey { secret_key } => PrivateKey::from_string(secret_key)?,
            Secret::SeedPhrase { seed_phrase } => PrivateKey::from_payload(
                &sep5::SeedPhrase::from_str(seed_phrase)?
                    .from_path_index(index.unwrap_or_default(), None)?
                    .private()
                    .0,
            )?,
            Secret::Ledger => panic!("Ledger does not reveal secret key"),
            Secret::SecureStore { .. } => {
                return Err(Error::SecureStoreDoesNotRevealSecretKey);
            }
        })
    }

    pub fn public_key(&self, index: Option<usize>) -> Result<PublicKey, Error> {
        if let Secret::SecureStore {
            entry_name,
            cached_entry,
        } = self
        {
            let entry = Self::cached_secure_store_entry(index, entry_name, cached_entry)?;
            Ok(entry.get_public_key()?)
        } else {
            let key = self.key_pair(index)?;
            Ok(stellar_strkey::ed25519::PublicKey::from_payload(
                key.verifying_key().as_bytes(),
            )?)
        }
    }

    pub async fn signer(&self, hd_path: Option<usize>, print: Print) -> Result<Signer, Error> {
        let kind = match self {
            Secret::SecretKey { .. } | Secret::SeedPhrase { .. } => {
                let key = self.key_pair(hd_path)?;
                SignerKind::Local(LocalKey { key })
            }
            Secret::Ledger => {
                let hd_path: u32 = hd_path
                    .unwrap_or_default()
                    .try_into()
                    .expect("usize bigger than u32");
                SignerKind::Ledger(ledger::new(hd_path).await?)
            }
            Secret::SecureStore {
                entry_name,
                cached_entry,
            } => {
                let entry = Self::cached_secure_store_entry(hd_path, entry_name, cached_entry)?;
                SignerKind::SecureStore(entry.clone())
            }
        };
        Ok(Signer { kind, print })
    }

    fn cached_secure_store_entry(
        hd_path: Option<usize>,
        entry_name: &String,
        cached_entry: &Arc<OnceLock<SecureStoreEntry>>,
    ) -> Result<SecureStoreEntry, Error> {
        let entry = if let Some(e) = cached_entry.get() {
            e.clone()
        } else {
            let e = SecureStoreEntry::new(entry_name.clone(), hd_path)?;
            // It's fine if set fails because another thread initialized it concurrently.
            let _ = cached_entry.set(e.clone());
            e
        };
        Ok(entry)
    }

    pub fn key_pair(&self, index: Option<usize>) -> Result<ed25519_dalek::SigningKey, Error> {
        Ok(utils::into_signing_key(&self.private_key(index)?))
    }

    pub fn from_seed(seed: Option<&str>) -> Result<Self, Error> {
        Ok(seed_phrase_from_seed(seed)?.into())
    }
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
}
