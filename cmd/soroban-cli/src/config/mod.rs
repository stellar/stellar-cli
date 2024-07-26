use std::path::PathBuf;

use clap::{arg, command};
use secret::StellarSigner;
use serde::{Deserialize, Serialize};
use stellar_strkey::ed25519::PublicKey;

use crate::signer;
use crate::xdr::{Transaction, TransactionEnvelope};
use crate::{signer::Stellar, Pwd};

use self::network::Network;

pub mod alias;
pub mod data;
pub mod locator;
pub mod network;
pub mod secret;
pub mod sign_with;

#[derive(thiserror::Error, Debug)]
pub enum Error {
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
    /// Account that signs the final transaction. Alias `source`. Can be an identity (--source alice), a secret key (--source SC36…), or a seed phrase (--source "kite urban…").
    pub source_account: String,

    #[command(flatten)]
    pub locator: locator::Args,

    #[command(flatten)]
    pub sign_with: sign_with::Args,
}

impl Args {
    pub fn signer(&self) -> Result<StellarSigner, Error> {
        let (account, prompt) = self.sign_with.sign_with_key.as_ref().map_or_else(
            || (&self.source_account, false),
            |s| (s, !self.sign_with.yes),
        );
        Ok(self
            .locator
            .account(account)?
            .signer(self.sign_with.hd_path, prompt)?)
    }

    pub fn key_pair(&self) -> Result<ed25519_dalek::SigningKey, Error> {
        let key = self.locator.account(&self.source_account)?;
        Ok(key.key_pair(self.sign_with.hd_path)?)
    }

    pub async fn public_key(&self) -> Result<PublicKey, Error> {
        Ok(self.signer()?.get_public_key().await?)
    }

    pub async fn sign(&self, tx: Transaction) -> Result<TransactionEnvelope, Error> {
        let signer = self.signer()?;
        self.sign_with_signer(&signer, tx).await
    }

    pub async fn sign_with_signer(
        &self,
        signer: &(impl Stellar + std::marker::Sync),
        tx: Transaction,
    ) -> Result<TransactionEnvelope, Error> {
        let network = self.get_network()?;
        Ok(signer.sign_txn(tx, &network).await?)
    }

    pub async fn sign_soroban_authorizations(
        &self,
        tx: &Transaction,
        ledgers_from_current: u32,
    ) -> Result<Option<Transaction>, Error> {
        self.sign_soroban_authorizations_with_signer(&self.signer()?, tx, ledgers_from_current)
            .await
    }
    pub async fn sign_soroban_authorizations_with_signer(
        &self,
        signer: &(impl Stellar + std::marker::Sync),
        tx: &Transaction,
        ledgers_from_current: u32,
    ) -> Result<Option<Transaction>, Error> {
        let network = self.get_network()?;
        let client = crate::rpc::Client::new(&network.rpc_url)?;
        let expiration_ledger = client.get_latest_ledger().await?.sequence + ledgers_from_current;
        Ok(signer
            .sign_soroban_authorizations(tx, &network, expiration_ledger)
            .await?)
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
