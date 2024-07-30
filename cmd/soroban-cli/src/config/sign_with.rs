use std::path::PathBuf;

use crate::{
    signer::{self, Stellar},
    xdr::{Transaction, TransactionEnvelope},
};
use clap::arg;

use super::{
    locator,
    network::{self, Network},
    secret::{self, StellarSigner},
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Network(#[from] network::Error),
    #[error(transparent)]
    Signer(#[from] signer::Error),
    #[error(transparent)]
    Secret(#[from] secret::Error),
    #[error(transparent)]
    Locator(#[from] locator::Error),
    #[error(transparent)]
    Rpc(#[from] soroban_rpc::Error),
    #[error("No sign with key provided")]
    NoSignWithKey,
}

#[derive(Debug, clap::Args, Clone, Default)]
#[group(skip)]
pub struct Args {
    /// Sign with account. Alias `source`. Can be an identity (--source alice), a secret key (--source SC36…), or a seed phrase (--source "kite urban…").
    #[arg(
        long,
        conflicts_with = "sign_with_laboratory",
        env = "STELLAR_SIGN_WITH_SECRET"
    )]
    pub sign_with_key: Option<String>,
    /// Sign with labratory
    #[arg(
        long,
        visible_alias = "sign-with-lab",
        conflicts_with = "sign_with_key",
        env = "STELLAR_SIGN_WITH_LABRATORY"
    )]
    pub sign_with_laboratory: bool,

    #[arg(long, conflicts_with = "sign_with_laboratory")]
    /// If using a seed phrase, which hierarchical deterministic path to use, e.g. `m/44'/148'/{hd_path}`. Example: `--hd-path 1`. Default: `0`
    pub hd_path: Option<usize>,

    /// If `--sign-with-*` is used this will remove requirement of being prompted to sign transactions
    #[arg(long)]
    pub yes: bool,

    #[command(flatten)]
    pub network: network::Args,

    #[command(flatten)]
    pub locator: locator::Args,
}

impl Args {
    pub fn signer(&self) -> Result<StellarSigner, Error> {
        let account = self.sign_with_key.as_deref().ok_or(Error::NoSignWithKey)?;
        Ok(self
            .locator
            .account(account)?
            .signer(self.hd_path, !self.yes)?)
    }

    pub async fn sign_txn_env(
        &self,
        tx: TransactionEnvelope,
    ) -> Result<TransactionEnvelope, Error> {
        let signer = self.signer()?;
        self.sign_tx_env_with_signer(&signer, tx).await
    }

    pub async fn sign_tx_env_with_signer(
        &self,
        signer: &(impl Stellar + std::marker::Sync),
        tx_env: TransactionEnvelope,
    ) -> Result<TransactionEnvelope, Error> {
        let network = self.get_network()?;
        Ok(signer.sign_txn_env(tx_env, &network).await?)
    }

    pub async fn sign_soroban_authorizations(
        &self,
        tx: &Transaction,
        expiration_ledger: u32,
    ) -> Result<Option<Transaction>, Error> {
        Ok(self
            .signer()?
            .sign_soroban_authorizations(tx, &self.get_network()?, expiration_ledger)
            .await?)
    }

    pub async fn sign_soroban_authorizations_with_signer(
        &self,
        signer: &(impl Stellar + std::marker::Sync),
        tx: &Transaction,
        expiration_ledger: u32,
    ) -> Result<Option<Transaction>, Error> {
        Ok(signer
            .sign_soroban_authorizations(tx, &self.get_network()?, expiration_ledger)
            .await?)
    }

    pub fn get_network(&self) -> Result<Network, Error> {
        eprintln!("{self:#?}");
        Ok(self.network.get(&self.locator)?)
    }

    pub fn config_dir(&self) -> Result<PathBuf, Error> {
        Ok(self.locator.config_dir()?)
    }
}
