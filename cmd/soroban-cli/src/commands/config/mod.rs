use clap::Parser;
use serde::{Deserialize, Serialize};
use soroban_ledger_snapshot::LedgerSnapshot;

use self::{network::Network, secret::Secret};

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
    /// S...          a seceret key
    /// alice         an identity
    /// 'kite urban.  a seed phrase
    /// DEFAULT       Is the key generated with `identity generate --seed 0000000000000000
    pub source_account: Option<String>,

    #[clap(long)]
    /// If using a seed phrase, which hd path to use, e.g. `m/44'/148'/{hd_path}`
    pub hd_path: Option<usize>,
}

impl Args {
    pub fn key_pair(&self) -> Result<ed25519_dalek::Keypair, Error> {
        let key = if let Some(source_account) = &self.source_account {
            Args::account(source_account)?
        } else {
            secret::Secret::test_seed_phrase()?
        };

        Ok(key.key_pair(self.hd_path)?)
    }

    pub fn account(account_str: &str) -> Result<Secret, Error> {
        if let Ok(secret) = locator::read_identity(account_str) {
            Ok(secret)
        } else {
            Ok(account_str.parse::<Secret>()?)
        }
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
