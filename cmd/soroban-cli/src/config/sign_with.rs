use std::path::PathBuf;

use crate::{
    signer::{self, sign_txn_env, Stellar},
    xdr::TransactionEnvelope,
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
    /// Sign with a local key. Can be an identity (--sign-with-key alice), a secret key (--sign-with-key SC36…), or a seed phrase (--sign-with-key "kite urban…"). If using seed phrase, `--hd-path` defaults to the `0` path.
    #[arg(
        long,
        conflicts_with = "sign_with_lab",
        env = "STELLAR_SIGN_WITH_SECRET"
    )]
    pub sign_with_key: Option<String>,
    /// Sign with laboratory
    #[arg(
        long,
        conflicts_with = "sign_with_key",
        env = "STELLAR_SIGN_WITH_LABORATORY",
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
        Ok(sign_txn_env(signer, tx_env, &network).await?)
    }

    pub async fn sign_tx_env_with_lab(&self, tx_env: TransactionEnvelope) -> Result<(), Error> {
        let network = self.get_network()?;
        let passphrase = network.network_passphrase;
        // fixme: this is not the correct way to encode url query strings, but this is how the version of zustand-querystring in lab is expecting it. zustand-querystring recently released an update, that _may_ fix this.
        let encoded_passphrase = passphrase.replace(" ", "%20").replace(";", "/;");
        let xdr = "AAAAAgAAAAC3g0zwH+GTFKaencL9HEX62fg4A2jjirzHdBH9cPvjCQAAAGQAEb7FAAAAAQAAAAEAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAEAAAABAAAAALeDTPAf4ZMUpp6dwv0cRfrZ+DgDaOOKvMd0Ef1w++MJAAAAAAAAAADcOHnq5sGLOngOCEMyLqqn5CvFV2HGbOSjJAIzhqBdkAAAAAA7msoAAAAAAAAAAAFw++MJAAAAQGVOS50rimyFFTxO0loZZ24n3FPSttnVHqvQQNZWkSgeHYywX6IGUqR6mBDCi7VQwgfNiACpLK7eySx2//SAjYw0=";
        let txn_sign_url = format!(
            "http://localhost:3000/transaction/sign?$=network$&passphrase={encoded_passphrase};&transaction$sign$activeView=overview&importXdr={xdr}"
        );

        open::that(txn_sign_url).unwrap(); //todo: handle unwrap

        Ok(())
    }

    pub fn get_network(&self) -> Result<Network, Error> {
        Ok(self.network.get(&self.locator)?)
    }

    pub fn config_dir(&self) -> Result<PathBuf, Error> {
        Ok(self.locator.config_dir()?)
    }
}
