use std::fmt::Debug;

use crate::commands::contract::{deploy, id};
use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
pub struct Root {
    #[clap(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Deploy a token contract to wrap an existing Stellar classic asset for smart contract usage
    /// Deprecated, use `soroban contract deploy asset` instead
    Wrap(deploy::asset::Cmd),
    /// Compute the expected contract id for the given asset
    /// Deprecated, use `soroban contract id asset` instead
    Id(id::asset::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Wrap(#[from] deploy::asset::Error),
    #[error(transparent)]
    Id(#[from] id::asset::Error),
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
