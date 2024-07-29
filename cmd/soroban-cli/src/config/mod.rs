use std::path::PathBuf;

use clap::{arg, command};
use secret::StellarSigner;
use serde::{Deserialize, Serialize};

use crate::signer::Stellar;
use crate::xdr::{Transaction, TransactionEnvelope};
use crate::Pwd;

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
    SignWith(#[from] sign_with::Error),
    #[error(transparent)]
    Secret(#[from] secret::Error),
    #[error(transparent)]
    Network(#[from] network::Error),
    #[error(transparent)]
    Locator(#[from] locator::Error),
}

#[derive(Debug, clap::Args, Clone, Default)]
#[group(skip)]
pub struct Args {
    #[arg(long, visible_alias = "source", env = "STELLAR_ACCOUNT")]
    /// Account that signs the final transaction. Alias `source`. Can be an identity (--source alice), a secret key (--source SC36…), or a seed phrase (--source "kite urban…").
    pub source_account: String,

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
            .sign_with
            .locator
            .account(account)?
            .signer(self.sign_with.hd_path, prompt)?)
    }

    pub async fn public_key(&self) -> Result<stellar_strkey::ed25519::PublicKey, Error> {
        Ok(self.sign_with.public_key().await?)
    }

    pub async fn sign(&self, tx: Transaction) -> Result<TransactionEnvelope, Error> {
        Ok(self.sign_with.sign_txn(tx).await?)
    }

    pub async fn sign_soroban_authorizations(
        &self,
        tx: &Transaction,
        ledgers_from_current: u32,
    ) -> Result<Option<Transaction>, Error> {
        Ok(self
            .sign_with
            .sign_soroban_authorizations(tx, ledgers_from_current)
            .await?)
    }

    pub async fn sign_soroban_authorizations_with_signer(
        &self,
        signer: &(impl Stellar + std::marker::Sync),
        tx: &Transaction,
        ledgers_from_current: u32,
    ) -> Result<Option<Transaction>, Error> {
        Ok(self
            .sign_with
            .sign_soroban_authorizations_with_signer(signer, tx, ledgers_from_current)
            .await?)
    }

    pub fn get_network(&self) -> Result<Network, Error> {
        Ok(self.sign_with.get_network()?)
    }

    pub fn config_dir(&self) -> Result<PathBuf, Error> {
        Ok(self.sign_with.config_dir()?)
    }

    pub fn resolve_contract_id(
        &self,
        contract_id: &str,
    ) -> Result<stellar_strkey::Contract, Error> {
        Ok(self
            .sign_with
            .locator
            .resolve_contract_id(contract_id, &self.get_network()?.network_passphrase)?)
    }

    pub fn save_contract_id(&self, contract_id: &str, alias: &str) -> Result<(), Error> {
        self.sign_with.locator.save_contract_id(
            &self.get_network()?.network_passphrase,
            contract_id,
            alias,
        )?;
        Ok(())
    }
}

impl Pwd for Args {
    fn set_pwd(&mut self, pwd: &std::path::Path) {
        self.sign_with.locator.set_pwd(pwd);
    }
}

#[derive(Default, Serialize, Deserialize)]
pub struct Config {}
