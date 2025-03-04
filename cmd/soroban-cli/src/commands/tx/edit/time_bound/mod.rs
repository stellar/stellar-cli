use super::global;

// mod clear;
mod set;

#[derive(Debug, clap::Subcommand)]
pub enum Cmd {
    /// Set the transaction time bound
    #[command(subcommand)]
    Set(set::Cmd),
    // /// Clear the transaction's memo.
    // Clear(clear::Cmd)
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Set(#[from] set::Error),
    // #[error(transparent)]
    // Clear(#[from] clear::Error)
}

impl Cmd {
    pub fn run(&self, global_args: &global::Args) -> Result<(), Error> { 
        match self {
            Cmd::Set(cmd) => cmd.run(global_args)?,
            // Cmd::Clear(cmd) => cmd.run(global_args)?
        };
        Ok(())
    }
}