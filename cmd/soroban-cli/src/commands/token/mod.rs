pub mod args;
pub mod transfer;

use crate::commands::global;

#[derive(Debug, clap::Subcommand)]
pub enum Cmd {
    /// Transfer tokens from one account to another
    Transfer(transfer::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Transfer(#[from] transfer::Error),
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        match self {
            Cmd::Transfer(cmd) => cmd.run(global_args).await?,
        }
        Ok(())
    }
}
