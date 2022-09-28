use std::fmt::Debug;

use clap::{Parser, Subcommand};

pub mod create;
pub mod wrap;

#[derive(Parser, Debug)]
pub struct Root {
    #[clap(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Deploy an asset contract for a new asset
    Create(create::Cmd),
    /// Deploy an asset contract to wrap an existing Stellar classic asset for smart contract usage
    Wrap(wrap::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Create(#[from] create::Error),
    #[error(transparent)]
    Wrap(#[from] wrap::Error),
}

impl Root {
    pub fn run(&self) -> Result<(), Error> {
        Ok(match &self.cmd {
            Cmd::Create(create) => create.run()?,
            Cmd::Wrap(wrap) => wrap.run()?,
        })
    }
}
