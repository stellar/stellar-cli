use crate::commands::global;
use clap::Subcommand;

mod default;
mod stats;
mod unset;

#[derive(Debug, Subcommand)]
pub enum Cmd {
    /// Fetch the feestats from the network
    Stats(stats::Cmd),
    /// Set the default inclusion fee settings for the CLI
    #[command(name = "use")]
    Default(default::Cmd),
    /// Remove the default inclusion fee settings for the CLI
    #[command(name = "unset")]
    Unset(unset::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Stats(#[from] stats::Error),
    #[error(transparent)]
    Default(#[from] default::Error),
    #[error(transparent)]
    Unset(#[from] unset::Error),
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        match &self {
            Cmd::Stats(cmd) => cmd.run(global_args).await?,
            Cmd::Default(cmd) => cmd.run(global_args).await?,
            Cmd::Unset(cmd) => cmd.run(global_args).await?,
        }
        Ok(())
    }
}
