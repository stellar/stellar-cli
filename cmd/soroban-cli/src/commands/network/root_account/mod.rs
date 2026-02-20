use clap::Parser;
use sha2::{Digest, Sha256};

use crate::commands::global;
use crate::config::network::passphrase;
use crate::config::{locator, network};

pub mod public_key;
pub mod secret;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Config(#[from] locator::Error),
    #[error(transparent)]
    Network(#[from] network::Error),
    #[error(transparent)]
    Strkey(#[from] stellar_strkey::DecodeError),
}

/// Shared arguments for resolving the network passphrase and deriving the root account seed.
#[derive(Debug, clap::Parser, Clone)]
#[group(skip)]
pub struct Args {
    // @dev: `network` and `network-passphrase` args are provided explicitly as the `rpc-url` is not needed
    /// Network passphrase to decode the root account for
    #[arg(long = "network-passphrase", env = "STELLAR_NETWORK_PASSPHRASE")]
    pub network_passphrase: Option<String>,

    /// Name of network to use from config
    #[arg(long, short = 'n', env = "STELLAR_NETWORK")]
    pub network: Option<String>,

    #[command(flatten)]
    pub locator: locator::Args,
}

impl Args {
    /// Resolve the network passphrase and derive the 32-byte key.
    pub fn root_key(&self) -> Result<[u8; 32], Error> {
        // If a user explicitly provides a network passphrase, use that.
        // Otherwise, look up the network with the typical resolution process and use its passphrase.
        let network_passphrase = match (self.network.as_deref(), self.network_passphrase.clone()) {
            // Fall back to testnet as the default network if no config default is set
            (None, None) => passphrase::TESTNET.to_string(),
            (Some(network), None) => self.locator.read_network(network)?.network_passphrase,
            (_, Some(network_passphrase)) => network_passphrase,
        };
        Ok(Sha256::digest(network_passphrase.as_bytes()).into())
    }
}

#[derive(Debug, Parser)]
pub enum Cmd {
    /// Output a network's root account address (public key)
    #[command(visible_alias = "address")]
    PublicKey(public_key::Cmd),

    /// Output a network's root account secret key
    Secret(secret::Cmd),
}

impl Cmd {
    pub fn run(&self, _global_args: &global::Args) -> Result<(), Error> {
        match self {
            Cmd::PublicKey(cmd) => cmd.run()?,
            Cmd::Secret(cmd) => cmd.run()?,
        }
        Ok(())
    }
}
