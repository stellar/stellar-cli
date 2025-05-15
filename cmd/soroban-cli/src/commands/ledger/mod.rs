pub mod entry;

use clap::Parser;

#[derive(Debug, Parser)]
pub enum Cmd {
    /// Work with ledger entries.
    #[command(subcommand)]
    Entry(entry::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Entry(#[from] entry::Error),
}

impl Cmd {
    pub async fn run(&self) -> Result<(), Error> {
        match self {
            Cmd::Entry(cmd) => cmd.run().await?,
        };
        Ok(())
    }
}
