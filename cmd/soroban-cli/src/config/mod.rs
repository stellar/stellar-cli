use clap::Parser;
use serde::{Deserialize, Serialize};
use soroban_ledger_snapshot::LedgerSnapshot;

use crate::utils;

use self::network::Network;

pub mod identity;
pub mod ledger_file;
pub mod locator;
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
    Ledger(#[from] ledger_file::Error),

    #[error(transparent)]
    Secret(#[from] secret::Error),

    #[error(transparent)]
    Config(#[from] locator::Error),

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

#[derive(Debug, clap::Args, Clone)]
pub struct Args {
    #[clap(flatten)]
    pub secrets: secret::Args,

    #[clap(flatten)]
    pub network: network::Args,

    #[clap(flatten)]
    pub ledger_file: ledger_file::Args,

    #[clap(long, alias = "as")]
    /// Use specified identity to sign transaction
    pub identity: Option<String>,
}

impl Args {
    pub fn key_pair(&self) -> Result<ed25519_dalek::Keypair, Error> {
        // TODO remove unwrap and provide error
        let key = self
            .secrets
            .read_secret()
            .or_else(|_| locator::read_identity(self.identity.as_ref().unwrap()))?;
        let str_key = match &key {
            secret::Secret::SecretKey { secret_key } => secret_key,
            secret::Secret::SeedPhrase { seed_phrase: _ } => {
                todo!("Still need to implement seedphrase")
            }
        };
        utils::parse_secret_key(str_key).map_err(|_| Error::CannotParseSecretKey)
    }

    pub fn get_network(&self) -> Result<Network, Error> {
        Ok(self.network.get_network()?)
    }

    pub fn is_no_network(&self) -> bool {
        self.network.network.is_none()
            && self.network.network_passphrase.is_none()
            && self.network.rpc_url.is_none()
    }

    pub fn get_state(&self) -> Result<LedgerSnapshot, Error> {
        Ok(self.ledger_file.read()?)
    }

    pub fn set_state(&self, state: &mut LedgerSnapshot) -> Result<(), Error> {
        Ok(self.ledger_file.write(state)?)
    }
}

#[derive(Default, Serialize, Deserialize)]
pub struct Config {}
