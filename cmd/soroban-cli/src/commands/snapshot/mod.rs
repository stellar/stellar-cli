use clap::Parser;

use super::global;

pub mod create;

/// Create and operate on ledger snapshots.
#[derive(Debug, Parser)]
pub enum Cmd {
    Create(create::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Create(#[from] create::Error),
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        match self {
            Cmd::Create(cmd) => cmd.run(global_args).await?,
        };
        Ok(())
    }
}
