use std::path::PathBuf;

use clap::arg;
use stellar_strkey::ed25519::PublicKey;

use super::network::{self, Network};
use super::{
    locator,
    secret::{self, StellarSigner},
};
use crate::{
    signer::{self, Stellar},
    xdr::{Transaction, TransactionEnvelope},
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
    /// Sign with secret key
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

    /// If `--sign-with-*` is used this will remove requirement of being prompted
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
