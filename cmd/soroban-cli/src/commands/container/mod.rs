use crate::commands::global;

pub(crate) mod logs;
pub(crate) mod shared;
pub(crate) mod start;
pub(crate) mod stop;
pub(crate) mod unset;
pub(crate) mod use_engine;

// TODO: remove once `network start` is removed
pub type StartCmd = start::Cmd;
// TODO: remove once `network top` is removed
pub type StopCmd = stop::Cmd;

#[derive(Debug, clap::Subcommand)]
pub enum Cmd {
    /// Get logs from a running network container
    Logs(logs::Cmd),
    /// Start a container running a Stellar node, RPC, API, and friendbot (faucet).
    ///
    /// `stellar container start NETWORK [OPTIONS]`
    ///
    /// By default, when starting a testnet container, without any optional arguments, it will run the equivalent of the following docker command:
    ///
    /// `docker run --rm -p 8000:8000 --name stellar stellar/quickstart:testing --testnet --enable rpc,horizon`
    Start(start::Cmd),
    /// Stop a network container started with `stellar container start`.
    Stop(stop::Cmd),
    /// Set the default container engine used by `stellar container` commands.
    Use(use_engine::Cmd),
    /// Unset the default container engine defined previously with `container use <engine>`.
    Unset(unset::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Logs(#[from] logs::Error),

    #[error(transparent)]
    Start(#[from] start::Error),

    #[error(transparent)]
    Stop(#[from] stop::Error),

    #[error(transparent)]
    Use(#[from] use_engine::Error),

    #[error(transparent)]
    Unset(#[from] unset::Error),
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        match &self {
            Cmd::Logs(cmd) => cmd.run(global_args).await?,
            Cmd::Start(cmd) => cmd.run(global_args).await?,
            Cmd::Stop(cmd) => cmd.run(global_args).await?,
            Cmd::Use(cmd) => cmd.run(global_args)?,
            Cmd::Unset(cmd) => cmd.run(global_args)?,
        }
        Ok(())
    }
}
