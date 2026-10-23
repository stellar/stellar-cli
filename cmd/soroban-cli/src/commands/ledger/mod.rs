use crate::commands::global;
use clap::Subcommand;
pub mod entry;
mod fetch;
mod latest;
pub mod meta;

#[derive(Debug, Subcommand)]
pub enum Cmd {
    /// Work with ledger entries.
    #[command(subcommand)]
    Entry(entry::Cmd),
    /// Get the latest ledger sequence and information from the network
    Latest(latest::Cmd),
    Fetch(fetch::Cmd),
    /// Work with ledger close metadata
    #[command(subcommand)]
    Meta(meta::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Entry(#[from] entry::Error),
    #[error(transparent)]
    Latest(#[from] latest::Error),
    #[error(transparent)]
    Fetch(#[from] fetch::Error),
    #[error(transparent)]
    Meta(#[from] meta::Error),
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        match &self {
            Cmd::Entry(cmd) => cmd.run().await?,
            Cmd::Latest(cmd) => cmd.run(global_args).await?,
            Cmd::Fetch(cmd) => cmd.run(global_args).await?,
            Cmd::Meta(cmd) => cmd.run(global_args).await?,
        }
        Ok(())
    }
}
