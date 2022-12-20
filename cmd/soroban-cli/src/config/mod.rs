use clap::Parser;
use serde::{Deserialize, Serialize};
use soroban_ledger_snapshot::LedgerSnapshot;

use crate::{utils, HEADING_RPC};

use self::network::Network;

pub mod identity;
pub mod ledger;
pub mod location;
pub mod network;
pub mod secret;

#[derive(Debug, Parser)]
pub enum Cmd {
    /// Configure different identities to sign transactions.
    #[clap(subcommand)]
    Identity(identity::Cmd),

    /// Configure different networks
    #[clap(subcommand)]
    Network(network::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Identity(#[from] identity::Error),

    #[error(transparent)]
    Network(#[from] network::Error),

    #[error(transparent)]
    Ledger(#[from] ledger::Error),

    #[error(transparent)]
    Secret(#[from] secret::Error),

    #[error(transparent)]
    Config(#[from] location::Error),

    #[error("cannot parse secret key")]
    CannotParseSecretKey,
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        match &self {
            Cmd::Identity(identity) => identity.run()?,
            Cmd::Network(network) => network.run()?,
        }
        Ok(())
    }
}

#[derive(Debug, clap::Args)]
pub struct Args {
    /// Secret key to sign the transaction sent to the rpc server
    #[clap(
            long = "secret-key",
            env = "SOROBAN_SECRET_KEY",
            help_heading = HEADING_RPC,
        )]
    secret_key: Option<String>,

    #[clap(flatten)]
    pub location: location::Args,

    #[clap(flatten)]
    pub network: network::Args,

    #[clap(flatten)]
    pub ledger: ledger::Args,
}

impl Args {
    pub fn key_pair(&self) -> Result<ed25519_dalek::Keypair, Error> {
        utils::parse_secret_key(self.secret_key.as_deref().unwrap())
            .map_err(|_| Error::CannotParseSecretKey)
    }

    pub fn get_network(&self) -> Result<Network, Error> {
        Ok(self.network.get_network(&self.location)?)
    }

    pub fn no_network(&self) -> bool {
        self.network.network.is_none()
            && self.network.network_passphrase.is_none()
            && self.network.rpc_url.is_none()
    }

    pub fn get_state(&self) -> Result<LedgerSnapshot, Error> {
        Ok(self.ledger.read()?)
    }

    pub fn set_state(&self, state: &mut LedgerSnapshot) -> Result<(), Error> {
        Ok(self.ledger.write(state)?)
    }
}

#[derive(Default, Serialize, Deserialize)]
pub struct Config {
    default_network: Option<String>,
    default_identity: Option<String>,
}
