use clap::Parser;

use super::global;

pub mod create;
pub mod merge;

/// Create and operate on ledger snapshots.
#[derive(Debug, Parser)]
pub enum Cmd {
    Create(create::Cmd),
    Merge(merge::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Create(#[from] create::Error),
    #[error(transparent)]
    Merge(#[from] merge::Error),
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        match self {
            Cmd::Create(cmd) => cmd.run(global_args).await?,
            Cmd::Merge(cmd) => cmd.run(global_args)?,
        }
        Ok(())
    }
}
