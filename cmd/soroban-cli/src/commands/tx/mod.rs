use super::global;

pub mod args;
pub mod hash;
pub mod new;
pub mod send;
pub mod sign;
pub mod simulate;
pub mod xdr;

pub use args::Args;

#[derive(Debug, clap::Subcommand)]
pub enum Cmd {
    /// Simulate a transaction envelope from stdin
    Simulate(simulate::Cmd),
    /// Calculate the hash of a transaction envelope from stdin
    Hash(hash::Cmd),
    /// Sign a transaction envelope appending the signature to the envelope
    Sign(sign::Cmd),
    /// Send a transaction envelope to the network
    Send(send::Cmd),
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
    #[error(transparent)]
    Sign(#[from] sign::Error),
    #[error(transparent)]
    Send(#[from] send::Error),
    #[error(transparent)]
    Args(#[from] args::Error),
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        match self {
            Cmd::Simulate(cmd) => cmd.run(global_args).await?,
            Cmd::Hash(cmd) => cmd.run(global_args)?,
            Cmd::New(cmd) => cmd.run(global_args).await?,
            Cmd::Sign(cmd) => cmd.run(global_args).await?,
            Cmd::Send(cmd) => cmd.run(global_args).await?,
        };
        Ok(())
    }
}
