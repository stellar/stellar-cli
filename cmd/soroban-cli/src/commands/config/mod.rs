use std::path::PathBuf;

use clap::{arg, command};
use serde::{Deserialize, Serialize};

use soroban_rpc::Client;
use stellar_strkey::Strkey;

use crate::{
    signer,
    xdr::{MuxedAccount, SequenceNumber, Transaction, TransactionEnvelope, Uint256},
    Pwd,
};

use self::{network::Network, secret::Secret};

use super::{keys, network};

pub mod data;
pub mod locator;
pub mod secret;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Identity(#[from] keys::Error),
    #[error(transparent)]
    Network(#[from] network::Error),
    #[error(transparent)]
    Secret(#[from] secret::Error),
    #[error(transparent)]
    Config(#[from] locator::Error),
    #[error(transparent)]
    Rpc(#[from] soroban_rpc::Error),
    #[error(transparent)]
    Signer(#[from] signer::Error),
}

#[derive(Debug, clap::Args, Clone, Default)]
#[group(skip)]
pub struct Args {
    #[command(flatten)]
    pub network: network::Args,

    #[arg(long, visible_alias = "source", env = "STELLAR_ACCOUNT")]
    /// Account that signs the final transaction. Alias `source`. Can be an identity (--source alice), a secret key (--source SC36…), or a seed phrase (--source "kite urban…"). Default: `identity generate --default-seed`
    pub source_account: String,

    #[arg(long)]
    /// If using a seed phrase, which hierarchical deterministic path to use, e.g. `m/44'/148'/{hd_path}`. Example: `--hd-path 1`. Default: `0`
    pub hd_path: Option<usize>,

    #[command(flatten)]
    pub locator: locator::Args,
}

impl Args {
    pub fn key_pair(&self) -> Result<ed25519_dalek::SigningKey, Error> {
        let key = self.account(&self.source_account)?;
        Ok(key.key_pair(self.hd_path)?)
    }

    pub async fn sign_with_local_key(&self, tx: Transaction) -> Result<TransactionEnvelope, Error> {
        self.sign(tx).await
    }

    pub async fn sign(&self, mut tx: Transaction) -> Result<TransactionEnvelope, Error> {
        let key = self.key_pair()?;
        let verifying_key = key.verifying_key().to_bytes();
        let account = Strkey::PublicKeyEd25519(stellar_strkey::ed25519::PublicKey(verifying_key));
        let network = self.get_network()?;
        let client = Client::new(&network.rpc_url)?;
        tx.seq_num = SequenceNumber(client.get_account(&account.to_string()).await?.seq_num.0 + 1);
        tx.source_account = MuxedAccount::Ed25519(Uint256(verifying_key));
        Ok(signer::sign_tx(&key, &tx, &network.network_passphrase)?)
    }

    pub async fn sign_soroban_authorizations(
        &self,
        tx: &Transaction,
        signers: &[ed25519_dalek::SigningKey],
    ) -> Result<Option<Transaction>, Error> {
        let network = self.get_network()?;
        let source_key = self.key_pair()?;
        let client = Client::new(&network.rpc_url)?;
        let latest_ledger = client.get_latest_ledger().await?.sequence;
        let seq_num = latest_ledger + 60; // ~ 5 min
        Ok(signer::sign_soroban_authorizations(
            tx,
            &source_key,
            signers,
            seq_num,
            &network.network_passphrase,
        )?)
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
