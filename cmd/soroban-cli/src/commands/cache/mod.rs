use clap::Parser;

pub mod clean;
pub mod info;
pub mod ls;
pub mod read;

#[derive(Debug, Parser)]
pub enum Cmd {
    /// List cached actions (transactions, simulations)
    Ls(ls::Cmd),
    /// Show location of cache
    Info(info::Cmd),
    /// Delete all cached actions
    Clean(clean::Cmd),
    /// Read cached action
    Read(read::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Info(#[from] info::Error),
    #[error(transparent)]
    Ls(#[from] ls::Error),
    #[error(transparent)]
    Clean(#[from] clean::Error),
    #[error(transparent)]
    Read(#[from] read::Error),
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        match self {
            Cmd::Ls(cmd) => cmd.run()?,
            Cmd::Info(cmd) => cmd.run()?,
            Cmd::Clean(cmd) => cmd.run()?,
            Cmd::Read(cmd) => cmd.run()?,
        };
        Ok(())
    }
}
