use super::global;

mod hash;
mod id;
mod memo_return;
mod text;

#[derive(Debug, clap::Subcommand)]
pub enum Cmd {
    /// Set the transaction memo text.
    #[command()]
    Text(text::Cmd),
    /// Set the transaction memo id
    #[command()]
    Id(id::Cmd),
    /// Set the transaction memo hash
    #[command()]
    Hash(hash::Cmd),
    /// Set the transaction memo return
    #[command()]
    Return(memo_return::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Text(#[from] text::Error),
    #[error(transparent)]
    Id(#[from] id::Error),
    #[error(transparent)]
    Hash(#[from] hash::Error),
    #[error(transparent)]
    Return(#[from] memo_return::Error),
}

impl Cmd {
    pub fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        match self {
            Cmd::Hash(cmd) => cmd.run(global_args)?,
            Cmd::Id(cmd) => cmd.run(global_args)?,
            Cmd::Return(cmd) => cmd.run(global_args)?,
            Cmd::Text(cmd) => cmd.run(global_args)?,
        };
        Ok(())
    }
}
