use clap::Parser;
use std::fmt::Debug;

use crate::commands::global;

mod envelope;
mod meta;
mod result;

#[derive(Debug, Parser)]
pub enum Cmd {
    Result(result::Cmd),
    Meta(meta::Cmd),
    Envelope(envelope::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Result(#[from] result::Error),
    #[error(transparent)]
    Meta(#[from] meta::Error),
    #[error(transparent)]
    Envelope(#[from] envelope::Error),
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        match self {
            Cmd::Result(cmd) => cmd.run(global_args).await?,
            Cmd::Meta(cmd) => cmd.run(global_args).await?,
            Cmd::Envelope(cmd) => cmd.run(global_args).await?,
        }
        Ok(())
    }
}
