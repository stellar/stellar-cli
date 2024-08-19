use clap::Parser;

use super::global;

pub mod create_account;

#[derive(Debug, Parser)]
pub enum Cmd {
    /// Simulate a transaction envelope from stdin
    CreateAccount(create_account::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    CreateAccount(#[from] create_account::Error),
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        match self {
            Cmd::CreateAccount(cmd) => cmd.run(global_args).await?,
        };
        Ok(())
    }
}
