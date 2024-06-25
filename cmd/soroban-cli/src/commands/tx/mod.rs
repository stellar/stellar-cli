use clap::Parser;

use super::global;

pub mod simulate;
pub mod xdr;

#[derive(Debug, Parser)]
pub enum Cmd {
    /// Simulate a transaction envelope from stdin
    Simulate(simulate::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// An error during the simulation
    #[error(transparent)]
    Simulate(#[from] simulate::Error),
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        match self {
            Cmd::Simulate(cmd) => cmd.run(global_args).await?,
        };
        Ok(())
    }
}
