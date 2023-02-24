use clap::Parser;
use serde::{Deserialize, Serialize};
use soroban_ledger_snapshot::LedgerSnapshot;

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

    #[error("--source-account requires a prefix `id:` or `sk:` found {0}")]
    MalformedSourceAccount(String),
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
    pub network: network::Args,

    #[clap(flatten)]
    pub ledger_file: ledger_file::Args,

    #[clap(long, alias = "source", env = "SOROBAN_ACCOUNT")]
    /// Account that signs the final transaction.
    /// sk:S...        Provides a seceret key
    /// id:alice    Provides an identity
    /// id:test     Is the key generated with `identity generate --seed 0000000000000000
    /// seed:one .. seed phrase
    pub source_account: Option<String>,

    #[clap(long)]
    /// If using a seed phrase, which hd path to use, e.g. `m/44'/148'/{hd_path}`
    pub hd_path: Option<usize>,
}

impl Args {
    pub fn key_pair(&self) -> Result<ed25519_dalek::Keypair, Error> {
        let key = if let Some(source_account) = &self.source_account {
            let parts = source_account
                .split_once(':')
                .ok_or_else(|| Error::MalformedSourceAccount(source_account.clone()))?;
            match parts {
                ("id", identity) => locator::read_identity(identity)?,
                ("sk", secret_key) => secret::Secret::SecretKey {
                    secret_key: secret_key.to_string(),
                },
                ("seed", seed_phrase) => secret::Secret::SeedPhrase {
                    seed_phrase: seed_phrase.to_string(),
                },
                _ => return Err(Error::MalformedSourceAccount(source_account.clone())),
            }
        } else if let Some(identity) = &self.identity {
            locator::read_identity(identity)?
        } else {
            secret::Secret::test_seed_phrase()?
        };

        Ok(key.key_pair(self.hd_path)?)
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
