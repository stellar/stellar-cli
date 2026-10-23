use crate::commands::global;
use clap::Subcommand;

pub mod fetch;

#[derive(Debug, Subcommand)]
pub enum Cmd {
    /// Download ledger close meta from a network's ledger meta datastore (SEP-54)
    Fetch(fetch::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Fetch(#[from] fetch::Error),
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        match self {
            Cmd::Fetch(cmd) => cmd.run(global_args).await?,
        }
        Ok(())
    }
}
