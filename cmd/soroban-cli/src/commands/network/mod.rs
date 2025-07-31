use super::{config::locator, global};
use clap::Parser;

pub mod add;
pub mod settings;
pub mod default;
pub mod health;
pub mod ls;
pub mod rm;

#[derive(Debug, Parser)]
pub enum Cmd {
    /// Add a new network
    Add(add::Cmd),

    /// Remove a network
    Rm(rm::Cmd),

    /// List networks
    Ls(ls::Cmd),

    /// Fetch the network's config settings
    Settings(settings::Cmd),

    /// ⚠️ Deprecated: use `stellar container start` instead
    ///
    /// Start network
    ///
    /// Start a container running a Stellar node, RPC, API, and friendbot (faucet).
    ///
    /// `stellar network start NETWORK [OPTIONS]`
    ///
    /// By default, when starting a testnet container, without any optional arguments, it will run the equivalent of the following docker command:
    ///
    /// `docker run --rm -p 8000:8000 --name stellar stellar/quickstart:testing --testnet --enable rpc,horizon`
    #[cfg(feature = "version_lt_23")]
    Start(crate::commands::container::StartCmd),

    /// ⚠️ Deprecated: use `stellar container stop` instead
    ///
    /// Stop a network started with `network start`. For example, if you ran `stellar network start local`, you can use `stellar network stop local` to stop it.
    #[cfg(feature = "version_lt_23")]
    Stop(crate::commands::container::StopCmd),

    /// Set the default network that will be used on all commands.
    /// This allows you to skip `--network` or setting a environment variable,
    /// while reusing this value in all commands that require it.
    #[command(name = "use")]
    Default(default::Cmd),

    /// ⚠️ Deprecated: use `stellar container` instead
    ///
    /// Commands to start, stop and get logs for a quickstart container
    #[cfg(feature = "version_lt_23")]
    #[command(subcommand)]
    Container(crate::commands::container::Cmd),

    /// Checks the health of the configured RPC
    Health(health::Cmd),
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
    ConfigSettings(#[from] settings::Error),

    #[error(transparent)]
    Health(#[from] health::Error),

    #[cfg(feature = "version_lt_23")]
    #[error(transparent)]
    Start(#[from] crate::commands::container::start::Error),

    #[cfg(feature = "version_lt_23")]
    #[error(transparent)]
    Stop(#[from] crate::commands::container::stop::Error),

    #[cfg(feature = "version_lt_23")]
    #[error(transparent)]
    Container(#[from] crate::commands::container::Error),
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        match self {
            Cmd::Default(cmd) => cmd.run(global_args)?,
            Cmd::Add(cmd) => cmd.run()?,
            Cmd::Rm(new) => new.run()?,
            Cmd::Ls(cmd) => cmd.run()?,
            Cmd::ConfigSettings(cmd) => cmd.run(global_args).await?,
            #[cfg(feature = "version_lt_23")]
            Cmd::Container(cmd) => cmd.run(global_args).await?,

            #[cfg(feature = "version_lt_23")]
            Cmd::Start(cmd) => {
                eprintln!("⚠️ Warning: `network start` has been deprecated. Use `container start` instead");
                cmd.run(global_args).await?;
            }
            #[cfg(feature = "version_lt_23")]
            Cmd::Stop(cmd) => {
                println!(
                    "⚠️ Warning: `network stop` has been deprecated. Use `container stop` instead"
                );
                cmd.run(global_args).await?;
            }
            Cmd::Health(cmd) => cmd.run(global_args).await?,
        }
        Ok(())
    }
}
