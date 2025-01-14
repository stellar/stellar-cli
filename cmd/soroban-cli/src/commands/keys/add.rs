use std::io::Write;

use clap::command;
use sep5::SeedPhrase;

use crate::{
    commands::global,
    config::{address::KeyName, locator, secret::{self, Secret}},
    print::Print, signer::secure_store::{self, SecureStore},
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Secret(#[from] secret::Error),

    #[error(transparent)]
    Config(#[from] locator::Error),

    #[error(transparent)]
    SecureStore(#[from] secure_store::Error),

    #[error(transparent)]
    SeedPhrase(#[from] sep5::error::Error),

    #[error("secret input error")]
    PasswordRead,
}

#[derive(Debug, clap::Parser, Clone)]
#[group(skip)]
pub struct Cmd {
    /// Name of identity
    pub name: KeyName,

    #[command(flatten)]
    pub secrets: secret::Args,

    #[command(flatten)]
    pub config_locator: locator::Args,
}

impl Cmd {
    pub fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let print = Print::new(global_args.quiet);
        let secret = self.read_secret()?;
        let path = self.config_locator.write_identity(&self.name, &secret)?;
        print.checkln(format!("Key saved with alias {:?} in {path:?}", self.name));
        Ok(())
    }

    fn read_secret(&self) -> Result<Secret, Error> {
        if let Ok(secret_key) = std::env::var("SOROBAN_SECRET_KEY") {
            return Ok(Secret::SecretKey { secret_key })
        }

        println!("Type a secret key or 12/24 word seed phrase:");
        let secret_key = read_password()?;
        if self.secrets.secure_store {
            let seed_phrase: SeedPhrase = secret_key.parse()?;
            let print = &Print::new(false);
            Ok(SecureStore::save_secret(print, &self.name, seed_phrase)?)
        } else {
            Ok(secret_key.parse()?)
        }
    }
}

fn read_password() -> Result<String, Error> {
    std::io::stdout().flush().map_err(|_| Error::PasswordRead)?;
    rpassword::read_password().map_err(|_| Error::PasswordRead)
}