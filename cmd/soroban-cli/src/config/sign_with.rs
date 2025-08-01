use crate::{
    config::UnresolvedMuxedAccount,
    print::Print,
    signer::{self, ledger, Signer, SignerKind},
    xdr::{self, TransactionEnvelope},
};
use clap::arg;

use super::{
    locator,
    network::{self, Network},
    secret,
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
    StrKey(#[from] stellar_strkey::DecodeError),
    #[error(transparent)]
    Xdr(#[from] xdr::Error),
    #[error(transparent)]
    Ledger(#[from] signer::ledger::Error),
}

#[derive(Debug, clap::Args, Clone, Default)]
#[group(skip)]
pub struct Args {
    /// Sign with a local key or key saved in OS secure storage. Can be an identity (--sign-with-key alice), a secret key (--sign-with-key SC36…), or a seed phrase (--sign-with-key "kite urban…"). If using seed phrase, `--hd-path` defaults to the `0` path.
    #[arg(long, env = "STELLAR_SIGN_WITH_KEY")]
    pub sign_with_key: Option<String>,

    #[arg(long, conflicts_with = "sign_with_lab")]
    /// If using a seed phrase to sign, sets which hierarchical deterministic path to use, e.g. `m/44'/148'/{hd_path}`. Example: `--hd-path 1`. Default: `0`
    pub hd_path: Option<usize>,

    #[allow(clippy::doc_markdown)]
    /// Sign with https://lab.stellar.org
    #[arg(long, conflicts_with = "sign_with_key", env = "STELLAR_SIGN_WITH_LAB")]
    pub sign_with_lab: bool,

    /// Sign with a ledger wallet
    #[arg(
        long,
        conflicts_with = "sign_with_key",
        conflicts_with = "sign_with_lab",
        env = "STELLAR_SIGN_WITH_LEDGER"
    )]
    pub sign_with_ledger: bool,
}

impl Args {
    // when a default_signer_account is provided, it will be used as the tx signer if the user does not specify a signer. The default signer should be the tx's source_account.
    pub async fn sign_tx_env(
        &self,
        tx: &TransactionEnvelope,
        locator: &locator::Args,
        network: &Network,
        quiet: bool,
        default_signer_account: Option<&UnresolvedMuxedAccount>,
    ) -> Result<TransactionEnvelope, Error> {
        let print = Print::new(quiet);
        let signer = if self.sign_with_lab {
            Signer {
                kind: SignerKind::Lab,
                print,
            }
        } else if self.sign_with_ledger {
            let ledger = ledger::new(
                self.hd_path
                    .unwrap_or_default()
                    .try_into()
                    .unwrap_or_default(),
            )
            .await?;
            Signer {
                kind: SignerKind::Ledger(ledger),
                print,
            }
        } else {
            // default to using the source account local key, if the user did not pass in a key
            let key_or_name = match self.sign_with_key.as_deref() {
                Some(k) => k,
                None => match default_signer_account {
                    Some(UnresolvedMuxedAccount::AliasOrSecret(ref s)) => s.as_str(),
                    _ => return Err(Error::NoSignWithKey),
                },
            };

            let secret = locator.get_secret_key(key_or_name)?;
            secret.signer(self.hd_path, print).await?
        };
        Ok(signer.sign_tx_env(tx, network).await?)
    }
}
