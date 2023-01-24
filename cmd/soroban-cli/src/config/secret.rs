use serde::{Deserialize, Serialize};
use std::io::Write;
use stellar_strkey::{ed25519::PrivateKey, ed25519::PublicKey};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("invalid secret key")]
    InvalidSecretKey,
    #[error("seed_phrase must be 12 words long, found {len}")]
    InvalidSeedPhrase { len: usize },
    #[error("seceret input error")]
    PasswordRead,
    #[error(transparent)]
    Secret(#[from] stellar_strkey::DecodeError),
}

#[derive(Debug, clap::Args, Clone)]
pub struct Args {
    /// Add using secret_key
    #[clap(long, conflicts_with = "seed-phrase")]
    pub secret_key: bool,

    /// Add using 12 word seed phrase to generate secret_key
    #[clap(long, conflicts_with = "secret-key")]
    pub seed_phrase: bool,
    // /// Use MacOS Keychain
    // #[clap(long)]
    // pub macos_keychain: bool,
}

impl Args {
    pub fn read_secret(&self) -> Result<Secret, Error> {
        if self.secret_key {
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
            if seed_phrase.len() != 12 {
                let len = seed_phrase.len();
                return Err(Error::InvalidSeedPhrase { len });
            }
            Ok(Secret::SeedPhrase {
                seed_phrase: seed_phrase
                    .into_iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(" "),
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
    // MacOS,
}

trait AsKey {
    fn public_key(&self) -> PublicKey;

    fn private_key(&self) -> PrivateKey;
}

fn read_password() -> Result<String, Error> {
    std::io::stdout().flush().map_err(|_| Error::PasswordRead)?;
    rpassword::read_password().map_err(|_| Error::PasswordRead)
}
