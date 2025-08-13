use clap::Parser;

pub mod ls;
pub mod read;

#[derive(Debug, Parser)]
pub enum Cmd {
    /// List cached actions (transactions, simulations)
    Ls(ls::Cmd),
    /// Read cached action
    Read(read::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Ls(#[from] ls::Error),
    #[error(transparent)]
    Read(#[from] read::Error),
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        match self {
            Cmd::Ls(cmd) => cmd.run()?,
            Cmd::Read(cmd) => cmd.run()?,
        }
        Ok(())
    }
}
