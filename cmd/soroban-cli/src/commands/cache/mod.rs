use clap::Parser;

pub mod actionlog;
pub mod clean;
pub mod path;

#[derive(Debug, Parser)]
pub enum Cmd {
    /// Delete the cache
    Clean(clean::Cmd),
    /// Show the location of the cache
    Path(path::Cmd),
    /// Access details about cached actions like transactions, and simulations.
    /// (Experimental. May see breaking changes at any time.)
    #[command(subcommand)]
    Actionlog(actionlog::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Clean(#[from] clean::Error),
    #[error(transparent)]
    Path(#[from] path::Error),
    #[error(transparent)]
    Actionlog(#[from] actionlog::Error),
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        match self {
            Cmd::Clean(cmd) => cmd.run()?,
            Cmd::Path(cmd) => cmd.run()?,
            Cmd::Actionlog(cmd) => cmd.run()?,
        }
        Ok(())
    }
}
