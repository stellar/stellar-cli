use std::fmt::Debug;

use clap::{Parser, Subcommand};

pub mod id;
pub mod wrap;

#[derive(Parser, Debug)]
pub struct Root {
    #[clap(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Deploy a token contract to wrap an existing Stellar classic asset for smart contract usage
    Wrap(wrap::Cmd),
    /// Compute the expected contract id for the given asset
    Id(id::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Wrap(#[from] wrap::Error),
    #[error(transparent)]
    Id(#[from] id::Error),
}

impl Root {
    pub async fn run(&self) -> Result<(), Error> {
        match &self.cmd {
            Cmd::Wrap(wrap) => wrap.run().await?,
            Cmd::Id(id) => id.run()?,
        }
        Ok(())
    }
}
