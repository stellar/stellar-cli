use clap::Parser;

use crate::rpc::{self};

use super::{config::locator, global};

pub mod add;
pub mod container;
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
    Start(container::StartCmd),
    /// ⚠️ Deprecated: use `stellar container stop` instead
    ///
    /// Stop a network started with `network start`. For example, if you ran `stellar network start local`, you can use `stellar network stop local` to stop it.
    Stop(container::StopCmd),

    /// Commands to start, stop and get logs for a quickstart container
    #[command(subcommand)]
    Container(container::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Add(#[from] add::Error),

    #[error(transparent)]
    Rm(#[from] rm::Error),

    #[error(transparent)]
    Ls(#[from] ls::Error),

    // TODO: remove once `network start` is removed
    #[error(transparent)]
    Start(#[from] container::start::Error),

    // TODO: remove once `network stop` is removed
    #[error(transparent)]
    Stop(#[from] container::stop::Error),

    #[error(transparent)]
    Container(#[from] container::Error),

    #[error(transparent)]
    Config(#[from] locator::Error),

    #[error("please provide a network; use --network or set SOROBAN_NETWORK env var")]
    Network,
    #[error(transparent)]
    Http(#[from] http::Error),
    #[error(transparent)]
    Rpc(#[from] rpc::Error),
    #[error(transparent)]
    Hyper(#[from] hyper::Error),
    #[error("Failed to parse JSON from {0}, {1}")]
    FailedToParseJSON(String, serde_json::Error),
    #[error("Invalid URL {0}")]
    InvalidUrl(String),
    #[error("Inproper response {0}")]
    InproperResponse(String),
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        match self {
            Cmd::Add(cmd) => cmd.run()?,
            Cmd::Rm(new) => new.run()?,
            Cmd::Ls(cmd) => cmd.run()?,
            Cmd::Container(cmd) => cmd.run(global_args).await?,

            // TODO Remove this once `network start` is removed
            Cmd::Start(cmd) => {
                eprintln!("⚠️ Warning: `network start` has been deprecated. Use `network container start` instead");
                cmd.run(global_args).await?;
            }
            // TODO Remove this once `network stop` is removed
            Cmd::Stop(cmd) => {
                println!("⚠️ Warning: `network stop` has been deprecated. Use `network container stop` instead");
                cmd.run(global_args).await?;
            }
        };
        Ok(())
    }
}
