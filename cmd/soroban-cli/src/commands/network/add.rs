use crate::commands::HEADING_RPC;
use crate::config::{locator, network, secret};
use clap::command;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Secret(#[from] secret::Error),

    #[error(transparent)]
    Config(#[from] locator::Error),
}

#[derive(Debug, clap::Parser, Clone)]
#[group(skip)]
pub struct Cmd {
    /// Name of network
    pub name: String,

    /// RPC server endpoint
    #[arg(
        long = "rpc-url",
        env = "STELLAR_RPC_URL",
        help_heading = HEADING_RPC,
    )]
    pub rpc_url: String,

    /// Network passphrase to sign the transaction sent to the rpc server
    #[arg(
            long,
            env = "STELLAR_NETWORK_PASSPHRASE",
            help_heading = HEADING_RPC,
        )]
    pub network_passphrase: String,

    #[command(flatten)]
    pub config_locator: locator::Args,
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        let network = network::Network {
            rpc_url: self.rpc_url.clone(),
            network_passphrase: self.network_passphrase.clone(),
            name: self.name.clone(),
        };

        Ok(self.config_locator.write_network(&self.name, &network)?)
    }
}
