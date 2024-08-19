use clap::Parser;

use super::global;

pub mod create_account;
pub mod payment;

#[derive(Debug, Parser)]
pub enum Cmd {
    /// Create a new account using another account
    CreateAccount(create_account::Cmd),
    /// Send a payment to an account
    Payment(payment::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    CreateAccount(#[from] create_account::Error),
    #[error(transparent)]
    Payment(#[from] payment::Error),
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        match self {
            Cmd::CreateAccount(cmd) => cmd.run(global_args).await?,
            Cmd::Payment(cmd) => cmd.run(global_args).await?,
        };
        Ok(())
    }
}
