pub mod identity;
use std::fmt::Debug;

use clap::Parser;

#[derive(Debug, Parser)]
pub enum Cmd {
    #[clap(subcommand)]
    Identity(identity::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Identity(#[from] identity::Error),
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        match &self {
            Cmd::Identity(identity) => identity.run()?,
        }
        Ok(())
    }
}
