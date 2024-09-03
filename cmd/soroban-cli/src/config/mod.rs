use std::path::PathBuf;

use clap::{arg, command};
use secret::StellarSigner;
use serde::{Deserialize, Serialize};

use crate::signer;
use crate::xdr::{Transaction, TransactionEnvelope};
use crate::Pwd;

use self::network::Network;

pub mod alias;
pub mod data;
pub mod locator;
pub mod network;
pub mod secret;
pub mod sign_with;
pub mod upgrade_check;

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
    #[error(transparent)]
    Signer(#[from] signer::Error),
}

#[derive(Debug, clap::Args, Clone, Default)]
#[group(skip)]
pub struct Args {
    #[arg(long, visible_alias = "source", env = "STELLAR_ACCOUNT")]
    /// Account where the final transaction originates from.
    /// If no `--sign-with-*` flag is passed, passed key will also be used to sign the transaction.
    /// Can be an identity (`--source alice`), a secret key (`--source SC36…`), or a seed phrase (`--source "kite urban…"`)
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

    pub async fn source_account(&self) -> Result<stellar_strkey::ed25519::PublicKey, Error> {
        Ok(self
            .sign_with
            .locator
            .account(&self.source_account)?
            .public_key(self.sign_with.hd_path)
            .await?)
    }

    pub async fn sign(&self, tx: Transaction) -> Result<TransactionEnvelope, Error> {
        Ok(self
            .sign_with
            .sign_tx_env_with_signer(&self.signer()?, tx.into())
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
