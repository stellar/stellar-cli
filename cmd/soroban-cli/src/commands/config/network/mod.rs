use std::str::FromStr;

use clap::{arg, Parser};
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

#[derive(Debug, clap::Args, Clone, Default)]
#[group(skip)]
pub struct Args {
    /// RPC server endpoint
    #[arg(
        long = "rpc-url",
        requires = "network_passphrase",
        env = "SOROBAN_RPC_URL",
        help_heading = HEADING_RPC,
    )]
    pub rpc_url: Option<String>,
    /// Network passphrase to sign the transaction sent to the rpc server
    #[arg(
        long = "network-passphrase",
        requires = "rpc_url",
        env = "SOROBAN_NETWORK_PASSPHRASE",
        help_heading = HEADING_RPC,
    )]
    pub network_passphrase: Option<String>,
    /// Helper URL to use for funding accounts on test networks
    #[arg(
        long = "helper-url",
        requires = "rpc_url",
        env = "SOROBAN_NETWORK_PASSPHRASE",
        help_heading = HEADING_RPC,
    )]
    pub helper_url: Option<String>,
    /// Name of network to use from config
    #[arg(
        long,
        conflicts_with = "network_passphrase",
        conflicts_with = "rpc_url",
        env = "SOROBAN_NETWORK",
        help_heading = HEADING_RPC,
    )]
    pub network: Option<String>,
}

impl Args {
    pub fn get(&self, locator: &locator::Args) -> Result<Network, Error> {
        if let Some(name) = self.network.as_deref() {
            Ok(locator.read_network(name)?)
        } else if let (Some(rpc_url), Some(network_passphrase)) =
            (self.rpc_url.clone(), self.network_passphrase.clone())
        {
            Ok(Network {
                rpc_url,
                network_passphrase,
                helper_url: self.helper_url.clone(),
            })
        } else {
            Err(Error::Network)
        }
    }

    pub fn is_no_network(&self) -> bool {
        self.network.is_none() && self.network_passphrase.is_none() && self.rpc_url.is_none()
    }
}

#[derive(Debug, clap::Args, Serialize, Deserialize, Clone)]
#[group(skip)]
pub struct Network {
    /// RPC server endpoint
    #[arg(
        long = "rpc-url",
        env = "SOROBAN_RPC_URL",
        help_heading = HEADING_RPC,
    )]
    pub rpc_url: String,
    /// Network passphrase to sign the transaction sent to the rpc server
    #[arg(
            long,
            env = "SOROBAN_NETWORK_PASSPHRASE",
            help_heading = HEADING_RPC,
        )]
    pub network_passphrase: String,

    /// Network passphrase to sign the transaction sent to the rpc server
    #[arg(
        long,
        env = "SOROBAN_HELPER_URL",
        help_heading = HEADING_RPC,
    )]
    pub helper_url: Option<String>,
}

impl Network {
    pub fn helper_url(&self, addr: &str) -> String {
        let authority = self.helper_url.clone().unwrap_or_else(|| {
            http::Uri::from_str(&self.rpc_url)
                .expect("Invalid URI")
                .into_parts()
                .authority
                .unwrap()
                .to_string()
        });
        tracing::trace!("helper url authority {:?}", authority);
        http::Uri::builder()
            .scheme("http")
            .authority(authority)
            .path_and_query(format!("/friendbot?addr={addr}"))
            .build()
            .expect("Invalid URI")
            .to_string()
    }
}

impl Network {
    pub fn futurenet() -> Self {
        Network {
            rpc_url: "https://rpc-futurenet.stellar.org:443".to_owned(),
            network_passphrase: "Test SDF Future Network ; October 2022".to_owned(),
            helper_url: Some("https://friendbot-futurenet.stellar.org".to_owned()),
        }
    }
}
