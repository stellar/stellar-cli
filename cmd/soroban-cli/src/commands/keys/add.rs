use clap::command;

use crate::config::{key, locator, secret};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Key(#[from] key::Error),

    #[error(transparent)]
    Config(#[from] locator::Error),
}

#[derive(Debug, clap::Parser, Clone)]
#[group(skip)]
pub struct Cmd {
    /// Name of identity
    pub name: String,

    #[command(flatten)]
    pub secrets: secret::Args,

    #[command(flatten)]
    pub config_locator: locator::Args,
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        Ok(self
            .config_locator
            .write_key(&self.name, &self.secrets.read_key()?)?)
    }
}
