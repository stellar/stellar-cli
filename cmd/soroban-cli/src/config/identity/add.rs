use std::io::Write;

use crate::utils;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Secret(#[from] secret::Error),
}

#[derive(Debug, clap::Args)]
pub struct Cmd {
    /// alias to associate with the profile
    pub alias: String,

    #[clap(flatten)]
    pub secrets: SecretArgs,

    /// Generate a new key pair and print seed phrase
    #[clap(long)]
    pub generate: bool,
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        println!("{self:#?}");

        println!("{:#?}", self.secrets.read_secret()?);
        Ok(())
    }
}

mod secret {
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
}

#[derive(Debug, clap::Args)]
pub struct SecretArgs {
    /// Add using secret_key
    #[clap(long)]
    pub secret_key: bool,

    /// Add using 12 word seed phrase to generate secret_key
    #[clap(long)]
    pub seed_phrase: bool,

    /// Use MacOS Keychain
    #[clap(long)]
    pub macos_keychain: bool,
}

impl SecretArgs {
    pub fn read_secret(&self) -> Result<Secret, secret::Error> {
        if self.secret_key {
            println!("Type a Secret Key: ");
            let secret_key = read_password()?;
            let key = utils::parse_secret_key(&secret_key)
                .map_err(|_| secret::Error::InvalidSecretKey)?;
            Ok(Secret::PrivateKey(key))
        } else if self.seed_phrase {
            println!("Type a 12 word seed phrase: ");
            let seed_phrase = read_password()?;
            let seed_phrase = seed_phrase.split_whitespace().collect::<Vec<&str>>();
            if seed_phrase.len() != 12 {
                let len = seed_phrase.len();
                return Err(secret::Error::InvalidSeedPhrase { len });
            }
            Ok(Secret::SeedPhrase(
                seed_phrase.into_iter().map(ToString::to_string).collect(),
            ))
        } else {
            Err(secret::Error::PasswordRead {})
        }
    }
}

#[derive(Debug)]
pub enum Secret {
    PrivateKey(ed25519_dalek::Keypair),
    SeedPhrase(Vec<String>),

    MacOS,
}

fn read_password() -> Result<String, secret::Error> {
    std::io::stdout()
        .flush()
        .map_err(|_| secret::Error::PasswordRead)?;
    rpassword::read_password().map_err(|_| secret::Error::PasswordRead)
}
