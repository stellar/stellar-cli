use crate::{signer, xdr::TransactionEnvelope};
use clap::arg;
use soroban_env_host::xdr::WriteXdr;
use soroban_sdk::xdr::Limits;
use url::Url;

use super::{
    locator,
    network::{self, Network},
    secret,
};

pub const LAB_URL: &str = "https://lab.stellar.org/transaction/cli-sign";

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
    StrKey(#[from] stellar_strkey::DecodeError),
    #[error(transparent)]
    Xdr(#[from] soroban_env_host::xdr::Error),
    #[error(transparent)]
    Url(#[from] url::ParseError),
    #[error(transparent)]
    Open(#[from] std::io::Error),
}

#[derive(Debug, clap::Args, Clone, Default)]
#[group(skip)]
pub struct Args {
    /// Sign with a local key. Can be an identity (--sign-with-key alice), a secret key (--sign-with-key SC36…), or a seed phrase (--sign-with-key "kite urban…"). If using seed phrase, `--hd-path` defaults to the `0` path.
    #[arg(long, env = "STELLAR_SIGN_WITH_KEY")]
    pub sign_with_key: Option<String>,

    #[arg(long, conflicts_with = "sign_with_lab")]
    /// If using a seed phrase to sign, sets which hierarchical deterministic path to use, e.g. `m/44'/148'/{hd_path}`. Example: `--hd-path 1`. Default: `0`
    pub hd_path: Option<usize>,

    /// Sign with <https://lab.stellar.org>
    #[arg(long, conflicts_with = "sign_with_key", env = "STELLAR_SIGN_WITH_LAB")]
    pub sign_with_lab: bool,
}

impl Args {
    pub fn sign_tx_env(
        &self,
        tx: TransactionEnvelope,
        locator: &locator::Args,
        network: &Network,
        quiet: bool,
    ) -> Result<TransactionEnvelope, Error> {
        let key_or_name = self.sign_with_key.as_deref().ok_or(Error::NoSignWithKey)?;
        let secret = locator.key(key_or_name)?;
        let signer = secret.signer(self.hd_path, quiet)?;
        Ok(signer.sign_tx_env(tx, network)?)
    }
    pub fn sign_tx_env_with_lab(
        &self,
        network: &Network,
        tx_env: &TransactionEnvelope,
        quiet: bool,
    ) -> Result<(), Error> {
        let passphrase = network.network_passphrase.clone();
        let xdr_buffer = tx_env.to_xdr_base64(Limits::none())?;
        let printer = crate::print::Print::new(quiet);

        let mut url = Url::parse(LAB_URL)?;
        url.query_pairs_mut()
            .append_pair("networkPassphrase", &passphrase)
            .append_pair("xdr", &xdr_buffer);

        let txn_sign_url = url.to_string();

        printer.println(format!("Opening lab to sign transaction: {txn_sign_url}"));
        open::that(txn_sign_url)?;

        Ok(())
    }
}
