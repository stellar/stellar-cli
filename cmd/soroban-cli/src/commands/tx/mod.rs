use super::global;

pub mod args;
pub mod hash;
pub mod new;
pub mod simulate;
pub mod xdr;

#[derive(Debug, clap::Subcommand)]
pub enum Cmd {
    /// Simulate a transaction envelope from stdin
    Simulate(simulate::Cmd),
    /// Calculate the hash of a transaction envelope from stdin
    Hash(hash::Cmd),
    /// Create a new transaction
    #[command(subcommand)]
    New(new::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Hash(#[from] hash::Error),
    #[error(transparent)]
    New(#[from] new::Error),
    #[error(transparent)]
    Simulate(#[from] simulate::Error),
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        match self {
            Cmd::Simulate(cmd) => cmd.run(global_args).await?,
            Cmd::Hash(cmd) => cmd.run(global_args)?,
            Cmd::New(cmd) => cmd.run(global_args).await?,
        };
        Ok(())
    }
}
