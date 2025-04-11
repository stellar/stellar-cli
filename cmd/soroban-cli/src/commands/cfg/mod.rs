mod migrate;

use clap::Parser;

/// Migrate config from previous versions.
#[derive(Debug, Parser)]
pub enum Cmd {
    Migrate(migrate::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Migrate(#[from] migrate::Error),
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        match self {
            Cmd::Migrate(cmd) => cmd.run()?
        };
        Ok(())
    }
}
