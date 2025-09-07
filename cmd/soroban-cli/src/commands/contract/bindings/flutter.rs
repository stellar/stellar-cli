use std::fmt::Debug;

use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("flutter binding generation is not implemented in the stellar-cli, but is available via the tool located here: https://github.com/lightsail-network/stellar-contract-bindings")]
    NotImplemented,
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        Err(Error::NotImplemented)
    }
}
