use super::global;

mod bump;

#[derive(Debug, clap::Subcommand)]
pub enum Cmd {
    /// Bump the transaction's sequence number
    #[command()]
    Bump(bump::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Bump(#[from] bump::Error),
}

impl Cmd {
    pub fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        match self {
            Cmd::Bump(cmd) => cmd.run(global_args)?,
        };
        Ok(())
    }
}