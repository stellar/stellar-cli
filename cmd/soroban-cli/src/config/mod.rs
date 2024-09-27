use std::path::PathBuf;

use clap::{arg, command};
use serde::{Deserialize, Serialize};

use crate::{
    print::Print,
    rpc_client::{Error as RpcClientError, RpcClient},
    signer::{self, LocalKey, Signer, SignerKind},
    xdr::{Transaction, TransactionEnvelope},
    Pwd,
};

use self::{network::Network, secret::Secret};

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
    Network(#[from] network::Error),
    #[error(transparent)]
    Secret(#[from] secret::Error),
    #[error(transparent)]
    Config(#[from] locator::Error),
    #[error(transparent)]
    Rpc(#[from] soroban_rpc::Error),
    #[error(transparent)]
    Signer(#[from] signer::Error),
    #[error(transparent)]
    StellarStrkey(#[from] stellar_strkey::DecodeError),
    #[error(transparent)]
    RpcClient(#[from] RpcClientError),
}

#[derive(Debug, clap::Args, Clone, Default)]
#[group(skip)]
pub struct Args {
    #[command(flatten)]
    pub network: network::Args,

    #[arg(long, visible_alias = "source", env = "STELLAR_ACCOUNT")]
    /// Account that where transaction originates from. Alias `source`.
    /// Can be an identity (--source alice), a public key (--source GDKW...),
    /// a secret key (--source SC36…), or a seed phrase (--source "kite urban…").
    /// If `--build-only` or `--sim-only` flags were NOT provided, this key will also be used to
    /// sign the final transaction. In that case, trying to sign with public key will fail.
    pub source_account: String,

    #[arg(long)]
    /// If using a seed phrase, which hierarchical deterministic path to use, e.g. `m/44'/148'/{hd_path}`. Example: `--hd-path 1`. Default: `0`
    pub hd_path: Option<usize>,

    #[command(flatten)]
    pub locator: locator::Args,
}

impl Args {
    // TODO: Replace PublicKey with MuxedAccount once https://github.com/stellar/rs-stellar-xdr/pull/396 is merged.
    pub fn source_account(&self) -> Result<stellar_strkey::ed25519::PublicKey, Error> {
        if let Ok(secret) = self.account(&self.source_account) {
            Ok(stellar_strkey::ed25519::PublicKey(
                secret.key_pair(self.hd_path)?.verifying_key().to_bytes(),
            ))
        } else {
            Ok(stellar_strkey::ed25519::PublicKey::from_string(
                &self.source_account,
            )?)
        }
    }

    pub fn key_pair(&self) -> Result<ed25519_dalek::SigningKey, Error> {
        let key = self.account(&self.source_account)?;
        Ok(key.key_pair(self.hd_path)?)
    }

    pub async fn sign_with_local_key(&self, tx: Transaction) -> Result<TransactionEnvelope, Error> {
        self.sign(tx).await
    }

    #[allow(clippy::unused_async)]
    pub async fn sign(&self, tx: Transaction) -> Result<TransactionEnvelope, Error> {
        let key = self.key_pair()?;
        let network = &self.get_network()?;
        let signer = Signer {
            kind: SignerKind::Local(LocalKey { key }),
            print: Print::new(false),
        };
        Ok(signer.sign_tx(tx, network)?)
    }

    pub async fn sign_soroban_authorizations(
        &self,
        tx: &Transaction,
        signers: &[ed25519_dalek::SigningKey],
    ) -> Result<Option<Transaction>, Error> {
        let network = self.get_network()?;
        let source_key = self.key_pair()?;
        let client = RpcClient::new(network.clone())?;
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
