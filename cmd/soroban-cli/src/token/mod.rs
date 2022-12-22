use std::fmt::Debug;

use clap::{Parser, Subcommand};

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
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Wrap(#[from] wrap::Error),
}

impl Root {
    pub async fn run(&self) -> Result<(), Error> {
        match &self.cmd {
            Cmd::Wrap(wrap) => wrap.run().await?,
        }
        Ok(())
    }
}
