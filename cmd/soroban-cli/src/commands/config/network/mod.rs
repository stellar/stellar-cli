use clap::Parser;
use serde::{Deserialize, Serialize};

use crate::commands::HEADING_RPC;

use super::locator;

pub mod add;
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
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Add(#[from] add::Error),

    #[error(transparent)]
    Rm(#[from] rm::Error),

    #[error(transparent)]
    Ls(#[from] ls::Error),

    #[error(transparent)]
    Config(#[from] locator::Error),

    #[error("network arg or rpc url  and network passphrase are required if using the network")]
    Network,
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        match self {
            Cmd::Add(cmd) => cmd.run()?,
            Cmd::Rm(new) => new.run()?,
            Cmd::Ls(cmd) => cmd.run()?,
        };
        Ok(())
    }
}

#[derive(Debug, clap::Args, Clone)]
pub struct Args {
    /// RPC server endpoint
    #[clap(
        long,
        requires = "network-passphrase",
        env = "SOROBAN_RPC_URL",
        help_heading = HEADING_RPC,
    )]
    pub rpc_url: Option<String>,
    /// Network passphrase to sign the transaction sent to the rpc server
    #[clap(
        long = "network-passphrase",
        requires = "rpc-url",
        env = "SOROBAN_NETWORK_PASSPHRASE",
        help_heading = HEADING_RPC,
    )]
    pub network_passphrase: Option<String>,
    /// Name of network to use from config
    #[clap(
        long,
        conflicts_with = "network-passphrase",
        conflicts_with = "rpc-url"
    )]
    pub network: Option<String>,
}

impl Args {
    pub fn get_network(&self) -> Result<Network, Error> {
        if let Some(name) = self.network.as_deref() {
            Ok(locator::read_network(name)?)
        } else if let (Some(rpc_url), Some(network_passphrase)) =
            (self.rpc_url.clone(), self.network_passphrase.clone())
        {
            Ok(Network {
                rpc_url,
                network_passphrase,
            })
        } else {
            Err(Error::Network)
        }
    }
}

#[derive(Debug, clap::Args, Serialize, Deserialize)]
pub struct Network {
    /// RPC server endpoint
    #[clap(
        long,
        env = "SOROBAN_RPC_URL",
        help_heading = HEADING_RPC,
    )]
    pub rpc_url: String,
    /// Network passphrase to sign the transaction sent to the rpc server
    #[clap(
            long,
            env = "SOROBAN_NETWORK_PASSPHRASE",
            help_heading = HEADING_RPC,
        )]
    pub network_passphrase: String,
}
