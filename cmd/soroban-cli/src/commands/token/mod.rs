pub mod approve;
pub mod args;
pub mod balance;
pub mod decimals;
pub mod name;
pub mod symbol;
pub mod transfer;

use crate::commands::global;

#[derive(Debug, clap::Subcommand)]
pub enum Cmd {
    /// Transfer tokens from one account to another
    Transfer(transfer::Cmd),

    /// Read the token balance of an account or contract
    Balance(balance::Cmd),

    /// Read the token's name (SEP-41 metadata)
    Name(name::Cmd),

    /// Read the token's symbol (SEP-41 metadata)
    Symbol(symbol::Cmd),

    /// Read the token's decimals (SEP-41 metadata)
    Decimals(decimals::Cmd),

    /// Approve an allowance for a spender to transfer on your behalf
    Approve(approve::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Transfer(#[from] transfer::Error),
    #[error(transparent)]
    Balance(#[from] balance::Error),
    #[error(transparent)]
    Name(#[from] name::Error),
    #[error(transparent)]
    Symbol(#[from] symbol::Error),
    #[error(transparent)]
    Decimals(#[from] decimals::Error),
    #[error(transparent)]
    Approve(#[from] approve::Error),
}

impl Error {
    /// Machine-readable discriminator for the JSON error envelope's `type` field.
    #[must_use]
    pub fn error_type(&self) -> &'static str {
        match self {
            Error::Transfer(e) => e.error_type(),
            Error::Balance(e) => e.error_type(),
            Error::Name(e) => e.error_type(),
            Error::Symbol(e) => e.error_type(),
            Error::Decimals(e) => e.error_type(),
            Error::Approve(e) => e.error_type(),
        }
    }
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        match self {
            Cmd::Transfer(cmd) => cmd.run(global_args).await?,
            Cmd::Balance(cmd) => cmd.run(global_args).await?,
            Cmd::Name(cmd) => cmd.run(global_args).await?,
            Cmd::Symbol(cmd) => cmd.run(global_args).await?,
            Cmd::Decimals(cmd) => cmd.run(global_args).await?,
            Cmd::Approve(cmd) => cmd.run(global_args).await?,
        }
        Ok(())
    }
}
