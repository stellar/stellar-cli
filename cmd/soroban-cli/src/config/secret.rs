use std::io::Write;

use crate::utils;

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

#[derive(Debug, clap::Args)]
pub struct Args {
    /// Add using secret_key
    #[clap(long)]
    pub secret_key: bool,

    /// Add using 12 word seed phrase to generate secret_key
    #[clap(long)]
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
            let key = utils::parse_secret_key(&secret_key).map_err(|_| Error::InvalidSecretKey)?;
            Ok(Secret::PrivateKey(key))
        } else if self.seed_phrase {
            println!("Type a 12 word seed phrase: ");
            let seed_phrase = read_password()?;
            let seed_phrase = seed_phrase.split_whitespace().collect::<Vec<&str>>();
            if seed_phrase.len() != 12 {
                let len = seed_phrase.len();
                return Err(Error::InvalidSeedPhrase { len });
            }
            Ok(Secret::SeedPhrase(
                seed_phrase.into_iter().map(ToString::to_string).collect(),
            ))
        } else {
            Err(Error::PasswordRead {})
        }
    }
}

#[derive(Debug)]
pub enum Secret {
    PrivateKey(ed25519_dalek::Keypair),
    SeedPhrase(Vec<String>),
    // MacOS,
}

fn read_password() -> Result<String, Error> {
    std::io::stdout().flush().map_err(|_| Error::PasswordRead)?;
    rpassword::read_password().map_err(|_| Error::PasswordRead)
}
