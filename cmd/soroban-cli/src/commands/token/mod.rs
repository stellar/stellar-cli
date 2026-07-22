pub mod args;
pub mod balance;
pub mod transfer;

use crate::commands::global;

#[derive(Debug, clap::Subcommand)]
pub enum Cmd {
    /// Transfer tokens from one account to another
    Transfer(transfer::Cmd),

    /// Read the token balance of an account or contract
    Balance(balance::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Transfer(#[from] transfer::Error),
    #[error(transparent)]
    Balance(#[from] balance::Error),
}

impl Error {
    /// Machine-readable discriminator for the JSON error envelope's `type` field.
    #[must_use]
    pub fn error_type(&self) -> &'static str {
        match self {
            Error::Transfer(e) => e.error_type(),
            Error::Balance(e) => e.error_type(),
        }
    }
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        match self {
            Cmd::Transfer(cmd) => cmd.run(global_args).await?,
            Cmd::Balance(cmd) => cmd.run(global_args).await?,
        }
        Ok(())
    }
}
