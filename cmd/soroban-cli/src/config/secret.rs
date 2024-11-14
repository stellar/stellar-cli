use clap::arg;
use serde::{Deserialize, Serialize};
use std::{io::Write, str::FromStr};
use stellar_strkey::ed25519::{PrivateKey, PublicKey};

use crate::{
    print::Print,
    signer::{self, LocalKey, Signer, SignerKind},
    utils,
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("invalid secret key")]
    InvalidSecretKey,
    // #[error("seed_phrase must be 12 words long, found {len}")]
    // InvalidSeedPhrase { len: usize },
    #[error("secret input error")]
    PasswordRead,
    #[error(transparent)]
    Secret(#[from] stellar_strkey::DecodeError),
    #[error(transparent)]
    SeedPhrase(#[from] sep5::error::Error),
    #[error(transparent)]
    Ed25519(#[from] ed25519_dalek::SignatureError),
    #[error("Invalid address {0}")]
    InvalidAddress(String),
    #[error(transparent)]
    Signer(#[from] signer::Error),
}

#[derive(Debug, clap::Args, Clone)]
#[group(skip)]
pub struct Args {
    /// Add using `secret_key`
    /// Can provide with `SOROBAN_SECRET_KEY`
    #[arg(long, conflicts_with_all = ["seed_phrase", "keychain"])]
    pub secret_key: bool,
    /// Add using 12 word seed phrase to generate `secret_key`
    #[arg(long, conflicts_with_all = ["secret_key", "keychain"])]
    pub seed_phrase: bool,

    /// Add using `keychain`
    #[arg(long, conflicts_with_all = ["seed_phrase", "secret_key"])]
    pub keychain: bool,
}

impl Args {
    pub fn read_secret(&self) -> Result<Secret, Error> {
        if let Ok(secret_key) = std::env::var("SOROBAN_SECRET_KEY") {
            Ok(Secret::SecretKey { secret_key })
        } else if self.secret_key {
            println!("Type a secret key: ");
            let secret_key = read_password()?;
            let secret_key = PrivateKey::from_string(&secret_key)
                .map_err(|_| Error::InvalidSecretKey)?
                .to_string();
            Ok(Secret::SecretKey { secret_key })
        } else if self.seed_phrase {
            println!("Type a 12 word seed phrase: ");
            let seed_phrase = read_password()?;
            let seed_phrase: Vec<&str> = seed_phrase.split_whitespace().collect();
            // if seed_phrase.len() != 12 {
            //     let len = seed_phrase.len();
            //     return Err(Error::InvalidSeedPhrase { len });
            // }
            Ok(Secret::SeedPhrase {
                seed_phrase: seed_phrase
                    .into_iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(" "),
            })
        } else if self.keychain {
            // generate a secret, and save it in the keychain
            // return a new type of secret?
            // for now, put it all in here
            println!("generate a secret in the keychain");
            // let keychain = keyring::Keyring::new("
            Ok(Secret::SecretKey {
                secret_key: "test".to_owned(),
            })
        } else {
            Err(Error::PasswordRead {})
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Secret {
    SecretKey { secret_key: String },
    SeedPhrase { seed_phrase: String },
    Keychain,
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
        } else if s == "keychain" {
            Ok(Secret::Keychain)
        } else {
            Err(Error::InvalidAddress(s.to_string()))
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
            Secret::Keychain => panic!("Keychain does not reveal secret key"),
        })
    }

    pub fn public_key(&self, index: Option<usize>) -> Result<PublicKey, Error> {
        let key = self.key_pair(index)?;
        Ok(stellar_strkey::ed25519::PublicKey::from_payload(
            key.verifying_key().as_bytes(),
        )?)
    }

    pub fn signer(&self, index: Option<usize>, print: Print) -> Result<Signer, Error> {
        let kind = match self {
            Secret::SecretKey { .. } | Secret::SeedPhrase { .. } => {
                let key = self.key_pair(index)?;
                SignerKind::Local(LocalKey { key })
            }
            Secret::Keychain => todo!(),
        };
        Ok(Signer { kind, print })
    }

    pub fn key_pair(&self, index: Option<usize>) -> Result<ed25519_dalek::SigningKey, Error> {
        Ok(utils::into_signing_key(&self.private_key(index)?))
    }

    pub fn from_seed(seed: Option<&str>) -> Result<Self, Error> {
        let seed_phrase = if let Some(seed) = seed.map(str::as_bytes) {
            sep5::SeedPhrase::from_entropy(seed)
        } else {
            sep5::SeedPhrase::random(sep5::MnemonicType::Words12)
        }?
        .seed_phrase
        .into_phrase();
        Ok(Secret::SeedPhrase { seed_phrase })
    }

    pub fn test_seed_phrase() -> Result<Self, Error> {
        Self::from_seed(Some("0000000000000000"))
    }
}

fn read_password() -> Result<String, Error> {
    std::io::stdout().flush().map_err(|_| Error::PasswordRead)?;
    rpassword::read_password().map_err(|_| Error::PasswordRead)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_SECRET_KEY: &str = "SBF5HLRREHMS36XZNTUSKZ6FTXDZGNXOHF4EXKUL5UCWZLPBX3NGJ4BH";
    const TEST_SEED_PHRASE: &str =
        "depth decade power loud smile spatial sign movie judge february rate broccoli";

    #[test]
    fn test_secret_from_key() {
        let secret = Secret::from_str(TEST_SECRET_KEY).unwrap();
        // assert that it is a Secret::SecretKey
        match secret {
            Secret::SecretKey { secret_key: _ } => assert!(true),
            _ => assert!(false),
        }
        // assert that we can get the private key from it
        let private_key = secret.private_key(None).unwrap();
        assert_eq!(private_key.to_string(), TEST_SECRET_KEY);

        let signer = secret.signer(None, Print::new(false)).unwrap();
        println!("signer: {:?}", signer.kind);
    }

    #[test]
    fn test_secret_from_seed_phrase() {
        let secret = Secret::from_str(TEST_SEED_PHRASE).unwrap();
        match secret {
            Secret::SeedPhrase { seed_phrase: _ } => assert!(true),
            _ => assert!(false),
        }

        let private_key = secret.private_key(None).unwrap();
        assert_eq!(private_key.to_string(), TEST_SECRET_KEY);
    }

    #[test]
    fn test_ledger_secret() {
        let secret = Secret::from_str("ledger").unwrap();
        match secret {
            Secret::Ledger => assert!(true),
            _ => assert!(false),
        }
    }

    #[test]
    #[should_panic]
    fn test_ledger_secret_will_not_reveal_private_key() {
        let secret = Secret::from_str("ledger").unwrap();
        secret.private_key(None).unwrap();
    }

    #[test]
    fn test_keychain_secret() {
        let keychain_secret = Secret::from_str("keychain").unwrap();
        match keychain_secret {
            Secret::Keychain => assert!(true),
            _ => assert!(false),
        }
    }

    #[test]
    #[should_panic]
    fn test_keychain_secret_will_not_reveal_private_key() {
        let secret = Secret::from_str("keychain").unwrap();
        secret.private_key(None).unwrap();
    }

    #[test]
    fn test_secret_from_invalid_string() {
        let secret = Secret::from_str("invalid");
        assert!(secret.is_err());
    }
}
