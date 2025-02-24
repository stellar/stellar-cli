use super::global;

pub mod args;

pub mod edit;
pub mod hash;
pub mod help;
pub mod new;
pub mod op;
pub mod send;
pub mod set;
pub mod sign;
pub mod simulate;
pub mod xdr;

pub use args::Args;

#[derive(Debug, clap::Subcommand)]
pub enum Cmd {
    /// Calculate the hash of a transaction envelope from stdin
    Hash(hash::Cmd),
    /// Create a new transaction
    #[command(subcommand)]
    New(new::Cmd),
    #[command(subcommand)]
    Edit(edit::Cmd),
    /// Set various options for a transaction
    Set(set::Cmd),
    /// Manipulate the operations in a transaction, including adding new operations
    #[command(subcommand, visible_alias = "op")]
    Operation(op::Cmd),
    /// Send a transaction envelope to the network
    Send(send::Cmd),
    /// Sign a transaction envelope appending the signature to the envelope
    Sign(sign::Cmd),
    /// Simulate a transaction envelope from stdin
    Simulate(simulate::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Edit(#[from] edit::Error),
    #[error(transparent)]
    Hash(#[from] hash::Error),
    #[error(transparent)]
    New(#[from] new::Error),
    #[error(transparent)]
    Op(#[from] op::Error),
    #[error(transparent)]
    Send(#[from] send::Error),
    #[error(transparent)]
    Sign(#[from] sign::Error),
    #[error(transparent)]
    Set(#[from] set::Error),
    #[error(transparent)]
    Args(#[from] args::Error),
    #[error(transparent)]
    Simulate(#[from] simulate::Error),
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        match self {
            Cmd::Edit(cmd) => cmd.run(global_args)?,
            Cmd::Hash(cmd) => cmd.run(global_args)?,
            Cmd::New(cmd) => cmd.run(global_args).await?,
            Cmd::Operation(cmd) => cmd.run(global_args).await?,
            Cmd::Send(cmd) => cmd.run(global_args).await?,
            Cmd::Set(cmd) => cmd.run(global_args)?,
            Cmd::Sign(cmd) => cmd.run(global_args).await?,
            Cmd::Simulate(cmd) => cmd.run(global_args).await?,
        };
        Ok(())
    }
}
