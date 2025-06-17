use crate::commands::global;
use clap::Subcommand;
mod latest;

#[derive(Debug, Subcommand)]
pub enum Cmd {
    /// Get the latest ledger sequence formation from the network
    Latest(latest::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Latest(#[from] latest::Error),
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        match &self {
            Cmd::Latest(cmd) => cmd.run(global_args).await?,
        }
        Ok(())
    }
}
