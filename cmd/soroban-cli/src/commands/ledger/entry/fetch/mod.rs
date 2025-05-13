use std::fmt::Debug;
use clap::Parser;

pub mod account;
pub mod contract;

#[derive(Debug, Parser)]
pub enum Cmd {
    Account(account::Cmd),
    Contract(contract::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Account(#[from] account::Error),
    #[error(transparent)]
    Contract(#[from] contract::Error),
}

impl Cmd {
    pub async fn run(&self) -> Result<(), Error> {
        match self {
            Cmd::Account(cmd) => cmd.run().await?,
            Cmd::Contract(cmd) => cmd.run().await?,
        }
        Ok(())
    }
}
