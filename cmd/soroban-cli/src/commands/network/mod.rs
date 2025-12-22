use super::{config::locator, global};
use clap::Parser;

pub mod add;
pub mod default;
pub mod health;
pub mod info;
pub mod ls;
pub mod rm;
pub mod settings;
pub mod unset;

#[derive(Debug, Parser)]
pub enum Cmd {
    /// Add a new network
    Add(add::Cmd),

    /// Remove a network
    Rm(rm::Cmd),

    /// List networks
    Ls(ls::Cmd),

    /// Set the default network that will be used on all commands.
    /// This allows you to skip `--network` or setting a environment variable,
    /// while reusing this value in all commands that require it.
    #[command(name = "use")]
    Default(default::Cmd),

    /// Fetch the health of the configured RPC
    Health(health::Cmd),

    /// Checks the health of the configured RPC
    Info(info::Cmd),

    /// Fetch the network's config settings
    Settings(settings::Cmd),

    /// Unset the default network defined previously with `network use <network>`
    Unset(unset::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Default(#[from] default::Error),

    #[error(transparent)]
    Add(#[from] add::Error),

    #[error(transparent)]
    Rm(#[from] rm::Error),

    #[error(transparent)]
    Ls(#[from] ls::Error),

    #[error(transparent)]
    Health(#[from] health::Error),

    #[error(transparent)]
    Info(#[from] info::Error),

    #[error(transparent)]
    Settings(#[from] settings::Error),

    #[error(transparent)]
    Unset(#[from] unset::Error),
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        match self {
            Cmd::Default(cmd) => cmd.run(global_args)?,
            Cmd::Add(cmd) => cmd.run()?,
            Cmd::Rm(new) => new.run()?,
            Cmd::Ls(cmd) => cmd.run()?,
            Cmd::Health(cmd) => cmd.run(global_args).await?,
            Cmd::Info(cmd) => cmd.run(global_args).await?,
            Cmd::Settings(cmd) => cmd.run(global_args).await?,
            Cmd::Unset(cmd) => cmd.run(global_args)?,
        }
        Ok(())
    }
}
