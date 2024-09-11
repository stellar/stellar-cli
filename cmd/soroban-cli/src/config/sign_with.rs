use crate::{
    signer::{self, sign_txn_env, Stellar},
    xdr::TransactionEnvelope,
};
use clap::arg;
use crossterm::event::{read, Event, KeyCode};
use soroban_env_host::xdr::WriteXdr;
use soroban_sdk::xdr::Limits;
use std::path::PathBuf;
use url::Url;

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
    #[error(transparent)]
    Xdr(#[from] soroban_env_host::xdr::Error),
    #[error(transparent)]
    Url(#[from] url::ParseError),
    #[error(transparent)]
    Open(#[from] std::io::Error),
    #[error("User cancelled signing, perhaps need to remove --check")]
    //todo pull this error into a common module instead of duplicating it here
    UserCancelledSigning,
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
    #[arg(long, conflicts_with = "sign_with_key", env = "STELLAR_SIGN_WITH_LAB")]
    pub sign_with_lab: bool,
    /// Lab URL for `sign_with_lab`
    #[arg(
        long,
        conflicts_with = "sign_with_key",
        env = "STELLAR_SIGN_WITH_LAB_URL",
        default_value = "https://lab.stellar.org/transaction/cli-sign"
    )]
    pub lab_url: String,

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

    pub fn sign_tx_env_with_lab(&self, tx_env: &TransactionEnvelope) -> Result<(), Error> {
        if !self.yes {
            //todo: bring this into a common mod instead of duplicating it here
            eprintln!("Press 'y' or 'Y' for yes, any other key for no:");
            match read_key() {
                'y' | 'Y' => {
                    eprintln!("Signing now...");
                }
                _ => return Err(Error::UserCancelledSigning),
            };
        }

        let passphrase = self.get_network()?.network_passphrase;
        let xdr_buffer = tx_env.to_xdr_base64(Limits::none())?;

        let mut url = Url::parse(&self.lab_url)?;
        url.query_pairs_mut()
            .append_pair("networkPassphrase", &passphrase)
            .append_pair("xdr", &xdr_buffer);

        let txn_sign_url = url.to_string();

        println!("Opening lab to sign transaction: {}", &txn_sign_url);
        open::that(txn_sign_url)?;

        Ok(())
    }

    pub fn get_network(&self) -> Result<Network, Error> {
        Ok(self.network.get(&self.locator)?)
    }

    pub fn config_dir(&self) -> Result<PathBuf, Error> {
        Ok(self.locator.config_dir()?)
    }
}

pub fn read_key() -> char {
    loop {
        if let Event::Key(key) = read().unwrap() {
            match key.code {
                KeyCode::Char(c) => return c,
                KeyCode::Esc => return '\x1b', // escape key
                _ => (),
            }
        }
    }
}
