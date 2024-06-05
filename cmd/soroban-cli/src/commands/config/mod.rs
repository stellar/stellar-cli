use std::path::PathBuf;

use clap::{arg, command};
use serde::{Deserialize, Serialize};

use soroban_rpc::Client;
use stellar_strkey::Strkey;

use crate::xdr::{MuxedAccount, SequenceNumber, Transaction, TransactionEnvelope, Uint256};
use crate::{
    signer::{LocalKey, Stellar},
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


    pub async fn sign_with_local_key(
        &self,
        tx: Transaction,
    ) -> Result<TransactionEnvelope, Error> {
        let signer = LocalKey::new(self.key_pair()?, false);
        self.sign(&signer, tx).await
    }

    pub async fn sign(
        &self,
        signer: &impl Stellar,
        mut tx: Transaction,
    ) -> Result<TransactionEnvelope, Error> {
        let key = signer.get_public_key().await.unwrap();
        let account = Strkey::PublicKeyEd25519(key);
        let network = self.get_network()?;
        let client = Client::new(&network.rpc_url)?;
        tx.seq_num = SequenceNumber(client.get_account(&account.to_string()).await?.seq_num.0 + 1);
        tx.source_account = MuxedAccount::Ed25519(Uint256(key.0));
        Ok(signer
            .sign_txn(tx, &network.network_passphrase)
            .await
            .unwrap())
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
