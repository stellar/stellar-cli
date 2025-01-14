use std::io::Write;

use clap::command;
use sep5::SeedPhrase;

use crate::{
    commands::global,
    config::{
        address::KeyName,
        locator,
        secret::{self, Secret},
    },
    print::Print,
    signer::secure_store,
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
        let secret = self.read_secret(&print)?;
        let path = self.config_locator.write_identity(&self.name, &secret)?;
        print.checkln(format!("Key saved with alias {:?} in {path:?}", self.name));
        Ok(())
    }

    fn read_secret(&self, print: &Print) -> Result<Secret, Error> {
        if let Ok(secret_key) = std::env::var("SOROBAN_SECRET_KEY") {
            Ok(Secret::SecretKey { secret_key })
        } else if self.secrets.secure_store {
            let prompt = "Type a 12 or 24 word seed phrase:";
            let secret_key = read_password(print, prompt)?;

            let seed_phrase: SeedPhrase = secret_key.parse()?;

            Ok(secure_store::save_secret(print, &self.name, seed_phrase)?)
        } else {
            let prompt = "Type a secret key or 12/24 word seed phrase:";
            let secret_key = read_password(print, prompt)?;
            Ok(secret_key.parse()?)
        }
    }
}

fn read_password(print: &Print, prompt: &str) -> Result<String, Error> {
    print.arrowln(prompt);
    std::io::stdout().flush().map_err(|_| Error::PasswordRead)?;
    rpassword::read_password().map_err(|_| Error::PasswordRead)
}
