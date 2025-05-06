use crate::commands::global;
use clap::Parser;

pub mod default;
pub mod ls;
pub mod search;

#[derive(Debug, Parser)]
pub enum Cmd {
    /// Search for for CLI plugins using GitHub
    Search(search::Cmd),

    /// List installed plugins
    Ls(ls::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Search(#[from] search::Error),

    #[error(transparent)]
    Ls(#[from] ls::Error),
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        match self {
            Cmd::Search(cmd) => cmd.run(global_args).await?,
            Cmd::Ls(cmd) => cmd.run()?,
        }
        Ok(())
    }
}
