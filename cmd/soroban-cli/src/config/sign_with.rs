use std::path::PathBuf;

use crate::{
    signer::{
        self,
        types::{sign_txn_env, Transaction},
    },
    xdr::TransactionEnvelope,
};
use clap::arg;
use stellar_strkey::ed25519::PublicKey;

use super::{
    locator,
    network::{self, Network},
    secret::{self, Secret},
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Network(#[from] network::Error),
    #[error(transparent)]
    Signer(#[from] signer::types::Error),
    #[error(transparent)]
    Secret(#[from] secret::Error),
    #[error(transparent)]
    Locator(#[from] locator::Error),
    #[error(transparent)]
    Rpc(#[from] soroban_rpc::Error),
    #[error("No sign with key provided")]
    NoSignWithKey,
    #[error(transparent)]
    StrKey(#[from] stellar_strkey::DecodeError),
}

#[derive(Debug, clap::Args, Clone, Default)]
#[group(skip)]
pub struct Args {
    /// Sign with a local key. Can be an identity (--sign-with-key alice), a secret key (--sign-with-key SC36…), or a seed phrase (--sign-with-key "kite urban…"). If using seed phrase, `--hd-path` defaults to the `0` path.
    #[arg(
        long,
        conflicts_with = "sign_with_lab",
        env = "STELLAR_SIGN_WITH_SECRET"
    )]
    pub sign_with_key: Option<String>,
    /// Sign with labratory
    #[arg(
        long,
        conflicts_with = "sign_with_key",
        env = "STELLAR_SIGN_WITH_LABRATORY",
        hide = true
    )]
    pub sign_with_lab: bool,

    #[arg(long, conflicts_with = "sign_with_lab")]
    /// If using a seed phrase to sign, sets which hierarchical deterministic path to use, e.g. `m/44'/148'/{hd_path}`. Example: `--hd-path 1`. Default: `0`
    pub hd_path: Option<usize>,

    /// If one of `--sign-with-*` flags is provided, don't ask to confirm to sign a transaction
    #[arg(long)]
    pub yes: bool,

    #[command(flatten)]
    pub network: network::Args,

    #[command(flatten)]
    pub locator: locator::Args,

    /// Source account of the transaction. By default will be the account that signs the transaction.
    #[arg(long, visible_alias = "source")]
    pub source_account: Option<String>,
}

impl Args {
    pub fn secret(&self) -> Result<Secret, Error> {
        let account = self.sign_with_key.as_deref().ok_or(Error::NoSignWithKey)?;
        Ok(self.locator.account(account)?)
    }

    pub async fn sign_txn_env(
        &self,
        tx: TransactionEnvelope,
    ) -> Result<TransactionEnvelope, Error> {
        let secret = self.secret()?;
        let signer = secret.signer(self.hd_path, !self.yes)?;
        let source_account = if let Some(source_account) = self.source_account.as_deref() {
            stellar_strkey::ed25519::PublicKey::from_string(source_account)?
        } else {
            secret.public_key(self.hd_path)?
        };

        self.sign_tx_env_with_signer(&signer, &source_account, tx)
            .await
    }

    pub async fn sign_tx_env_with_signer(
        &self,
        signer: &(impl Transaction + std::marker::Sync),
        source_account: &PublicKey,
        tx_env: TransactionEnvelope,
    ) -> Result<TransactionEnvelope, Error> {
        let network = self.get_network()?;
        Ok(sign_txn_env(signer, source_account, tx_env, &network).await?)
    }

    pub fn get_network(&self) -> Result<Network, Error> {
        Ok(self.network.get(&self.locator)?)
    }

    pub fn config_dir(&self) -> Result<PathBuf, Error> {
        Ok(self.locator.config_dir()?)
    }
}
