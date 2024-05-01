use clap::Parser;

pub mod actionlog;
pub mod clean;
pub mod info;

#[derive(Debug, Parser)]
pub enum Cmd {
    /// Access details about (transactions, simulations)
    #[command(subcommand)]
    Actionlog(actionlog::Cmd),
    /// Delete all cached actions
    Clean(clean::Cmd),
    /// Show location of cache
    Info(info::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Actionlog(#[from] actionlog::Error),
    #[error(transparent)]
    Clean(#[from] clean::Error),
    #[error(transparent)]
    Info(#[from] info::Error),
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        match self {
            Cmd::Actionlog(cmd) => cmd.run()?,
            Cmd::Info(cmd) => cmd.run()?,
            Cmd::Clean(cmd) => cmd.run()?,
        };
        Ok(())
    }
}
