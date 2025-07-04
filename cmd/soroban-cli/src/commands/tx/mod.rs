use super::global;

pub mod args;
pub mod edit;
pub mod fetch;
pub mod hash;
pub mod help;
pub mod new;
pub mod op;
pub mod send;
pub mod sign;
pub mod simulate;
pub mod update;
pub mod xdr;

pub use args::Args;

#[derive(Debug, clap::Subcommand)]
pub enum Cmd {
    /// Update the transaction
    #[command(subcommand)]
    Update(update::Cmd),
    /// Edit a transaction envelope from stdin. This command respects the environment variables
    /// `STELLAR_EDITOR`, `EDITOR` and `VISUAL`, in that order.
    ///
    /// Example: Start a new edit session
    ///
    /// $ stellar tx edit
    ///
    /// Example: Pipe an XDR transaction envelope
    ///
    /// $ stellar tx new manage-data --data-name hello --build-only | stellar tx edit
    ///
    Edit(edit::Cmd),
    /// Calculate the hash of a transaction envelope
    Hash(hash::Cmd),
    /// Create a new transaction
    #[command(subcommand)]
    New(new::Cmd),
    /// Manipulate the operations in a transaction, including adding new operations
    #[command(subcommand, visible_alias = "op")]
    Operation(op::Cmd),
    /// Send a transaction envelope to the network
    Send(send::Cmd),
    /// Sign a transaction envelope appending the signature to the envelope
    Sign(sign::Cmd),
    /// Simulate a transaction envelope from stdin
    Simulate(simulate::Cmd),
    /// Fetch a transaction from the network by hash
    /// If no subcommand is passed in, the transaction envelope will be returned
    Fetch(fetch::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Hash(#[from] hash::Error),
    #[error(transparent)]
    New(#[from] new::Error),
    #[error(transparent)]
    Edit(#[from] edit::Error),
    #[error(transparent)]
    Op(#[from] op::Error),
    #[error(transparent)]
    Send(#[from] send::Error),
    #[error(transparent)]
    Sign(#[from] sign::Error),
    #[error(transparent)]
    Args(#[from] args::Error),
    #[error(transparent)]
    Simulate(#[from] simulate::Error),
    #[error(transparent)]
    Update(#[from] update::Error),
    #[error(transparent)]
    Fetch(#[from] fetch::Error),
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        match self {
            Cmd::Hash(cmd) => cmd.run(global_args)?,
            Cmd::New(cmd) => cmd.run(global_args).await?,
            Cmd::Edit(cmd) => cmd.run(global_args)?,
            Cmd::Operation(cmd) => cmd.run(global_args).await?,
            Cmd::Send(cmd) => cmd.run(global_args).await?,
            Cmd::Sign(cmd) => cmd.run(global_args).await?,
            Cmd::Simulate(cmd) => cmd.run(global_args).await?,
            Cmd::Update(cmd) => cmd.run(global_args).await?,
            Cmd::Fetch(cmd) => cmd.run(global_args).await?,
        }
        Ok(())
    }
}
