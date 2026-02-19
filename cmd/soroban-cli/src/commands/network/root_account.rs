use sha2::{Digest, Sha256};
use stellar_strkey::ed25519::{PrivateKey, PublicKey};

use crate::commands::global;
use crate::config::network::passphrase;
use crate::config::{locator, network};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Config(#[from] locator::Error),
    #[error(transparent)]
    Network(#[from] network::Error),
    #[error(transparent)]
    Strkey(#[from] stellar_strkey::DecodeError),
}

#[derive(Debug, clap::Parser, Clone)]
#[group(skip)]
pub struct Cmd {
    // @dev: `network` and `network-passphrase` args are provided explicitly as the `rpc-url` is not needed
    /// Network passphrase to decode the root account for
    #[arg(long = "network-passphrase", env = "STELLAR_NETWORK_PASSPHRASE")]
    pub network_passphrase: Option<String>,

    /// Name of network to use from config
    #[arg(long, short = 'n', env = "STELLAR_NETWORK")]
    pub network: Option<String>,

    #[command(flatten)]
    pub locator: locator::Args,

    /// Output the public key (G... address)
    #[arg(long, conflicts_with = "secret")]
    pub public_key: bool,

    /// Output the secret key (S... key). This is the default behavior if neither `--public-key` nor `--secret` is provided.
    #[arg(long, conflicts_with = "public_key")]
    pub secret: bool,
}

impl Cmd {
    pub fn run(&self, _global_args: &global::Args) -> Result<(), Error> {
        // If a user explicitly provides a network passphrase, use that.
        // Otherwise, look up the network with the typical resolution process and use its passphrase.
        let network_passphrase = match (self.network.as_deref(), self.network_passphrase.clone()) {
            // Fall back to testnet as the default network if no config default is set
            (None, None) => passphrase::TESTNET.to_string(),
            (Some(network), None) => self.locator.read_network(network)?.network_passphrase,
            (_, Some(network_passphrase)) => network_passphrase,
        };
        let seed: [u8; 32] = Sha256::digest(network_passphrase.as_bytes()).into();

        if self.public_key {
            let signing_key = ed25519_dalek::SigningKey::from_bytes(&seed);
            let public_key = PublicKey::from_payload(signing_key.verifying_key().as_bytes())?;
            println!("{public_key}");
        } else {
            let private_key = PrivateKey::from_payload(&seed)?;
            println!("{private_key}");
        }

        Ok(())
    }
}
