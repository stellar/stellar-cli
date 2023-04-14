use std::path::PathBuf;

use clap::{arg, command, Parser};
use serde::{Deserialize, Serialize};
use soroban_ledger_snapshot::LedgerSnapshot;

use crate::Pwd;

use self::{network::Network, secret::Secret};

pub mod events_file;
pub mod identity;
pub mod ledger_file;
pub mod locator;
pub mod network;
pub mod secret;

#[derive(Debug, Parser)]
pub enum Cmd {
    /// Configure different identities to sign transactions.
    #[command(subcommand)]
    Identity(identity::Cmd),

    /// Configure different networks
    #[command(subcommand)]
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

#[derive(Debug, clap::Args, Clone, Default)]
#[group(skip)]
pub struct Args {
    #[command(flatten)]
    pub network: network::Args,

    #[command(flatten)]
    pub ledger_file: ledger_file::Args,

    #[arg(long, alias = "source", env = "SOROBAN_ACCOUNT")]
    /// Account that signs the final transaction. Alias `source`. Can be an identity (--source alice), a secret key (--source SC36…), or a seed phrase (--source "kite urban…"). Default: `identity generate --default-seed`
    pub source_account: Option<String>,

    #[arg(long)]
    /// If using a seed phrase, which hierarchical deterministic path to use, e.g. `m/44'/148'/{hd_path}`. Example: `--hd-path 1`. Default: `0`
    pub hd_path: Option<usize>,

    #[command(flatten)]
    pub locator: locator::Args,
}

impl Args {
    pub fn key_pair(&self) -> Result<ed25519_dalek::Keypair, Error> {
        let key = if let Some(source_account) = &self.source_account {
            self.account(source_account)?
        } else {
            secret::Secret::test_seed_phrase()?
        };

        Ok(key.key_pair(self.hd_path)?)
    }

    pub fn account(&self, account_str: &str) -> Result<Secret, Error> {
        if let Ok(secret) = self.locator.read_identity(account_str) {
            Ok(secret)
        } else {
            Ok(account_str.parse::<Secret>()?)
        }
    }

    pub fn get_network(&self) -> Result<Network, Error> {
        Ok(self.network.get(&self.locator)?)
    }

    pub fn is_no_network(&self) -> bool {
        self.network.is_no_network()
    }

    pub fn get_state(&self) -> Result<LedgerSnapshot, Error> {
        Ok(self.ledger_file.read(&self.locator.config_dir()?)?)
    }

    pub fn set_state(&self, state: &mut LedgerSnapshot) -> Result<(), Error> {
        Ok(self.ledger_file.write(state, &self.locator.config_dir()?)?)
    }

    pub fn config_dir(&self) -> Result<PathBuf, Error> {
        Ok(self.locator.config_dir()?)
    }
}

impl Pwd for Args {
    fn set_pwd(&mut self, pwd: &std::path::Path) {
        self.locator.set_pwd(pwd);
    }
}

#[derive(Default, Serialize, Deserialize)]
pub struct Config {}
