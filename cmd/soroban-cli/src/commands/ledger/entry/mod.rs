pub mod get;
use clap::Parser;

#[derive(Debug, Parser)]
pub enum Cmd {
    /// Get ledger entries.
    Get(get::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Get(#[from] get::Error),
}

impl Cmd {
    pub async fn run(&self) -> Result<(), Error> {
        match self {
            Cmd::Get(cmd) => cmd.run().await?,
        };
        Ok(())
    }
}