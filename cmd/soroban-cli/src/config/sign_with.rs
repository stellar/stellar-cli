use crate::{
    config::UnresolvedMuxedAccount,
    print::Print,
    signer::{self, ledger::LedgerEntry, Signer, SignerKind},
    xdr::{self, TransactionEnvelope},
};

use super::{
    locator,
    network::{self, Network},
    secret,
};
use crate::commands::HEADING_SIGNING;

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
    #[error("--sign-with-lab can't sign authorization entries; use --sign-with-key or --sign-with-ledger")]
    LabCannotSignAuthorization,
}

#[derive(Debug, clap::Args, Clone, Default)]
#[group(skip)]
pub struct Args {
    /// Sign with a local key or key saved in OS secure storage. Can be an identity (--sign-with-key alice), a secret key (--sign-with-key SC36…), or a seed phrase (--sign-with-key "kite urban…"). If using seed phrase, `--hd-path` defaults to the `0` path.
    #[arg(
        long,
        env = "STELLAR_SIGN_WITH_KEY",
        hide_env_values = true,
        help_heading = HEADING_SIGNING
    )]
    pub sign_with_key: Option<String>,

    #[arg(long, conflicts_with = "sign_with_lab", help_heading = HEADING_SIGNING)]
    /// If using a seed phrase to sign, sets which hierarchical deterministic path to use, e.g. `m/44'/148'/{hd_path}`. Example: `--hd-path 1`. Default: `0`
    pub hd_path: Option<u32>,

    #[allow(clippy::doc_markdown)]
    /// Sign with https://lab.stellar.org
    #[arg(
        long,
        conflicts_with = "sign_with_key",
        env = "STELLAR_SIGN_WITH_LAB",
        help_heading = HEADING_SIGNING
    )]
    pub sign_with_lab: bool,

    /// Sign with a ledger wallet
    #[arg(
        long,
        conflicts_with = "sign_with_key",
        conflicts_with = "sign_with_lab",
        env = "STELLAR_SIGN_WITH_LEDGER",
        help_heading = HEADING_SIGNING
    )]
    pub sign_with_ledger: bool,

    /// Sign without prompting for approval. Only applies to signatures that require user approval, like non-root Soroban auth entries.
    #[arg(long, help_heading = HEADING_SIGNING)]
    pub auto_sign: bool,
}

impl Args {
    /// The signer requested through the signing options for authorization
    /// entries, or `None` when no signing option was given.
    pub fn auth_signer(
        &self,
        locator: &locator::Args,
        print: &Print,
    ) -> Result<Option<Signer>, Error> {
        if self.sign_with_lab {
            return Err(Error::LabCannotSignAuthorization);
        }
        if self.sign_with_ledger {
            return Ok(Some(Signer {
                kind: SignerKind::Ledger(LedgerEntry {
                    hd_path: self.hd_path.unwrap_or_default(),
                    public_key: None,
                }),
                print: print.clone(),
            }));
        }
        let Some(key_or_name) = self.sign_with_key.as_deref() else {
            return Ok(None);
        };
        let secret = locator.get_secret_key_with_hd_path(key_or_name, self.hd_path)?;
        Ok(Some(secret.signer(self.hd_path, print.clone())?))
    }

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
            Signer {
                kind: SignerKind::Ledger(LedgerEntry {
                    hd_path: self.hd_path.unwrap_or_default(),
                    public_key: None,
                }),
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

            let secret = locator.get_secret_key_with_hd_path(key_or_name, self.hd_path)?;
            secret.signer(self.hd_path, print)?
        };
        Ok(signer.sign_tx_env(tx, network).await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secret_key() -> String {
        format!(
            "{}",
            stellar_strkey::ed25519::PrivateKey([7; 32]).as_unredacted()
        )
    }

    #[test]
    fn auth_signer_is_none_without_signing_options() {
        let signer = Args::default()
            .auth_signer(&locator::Args::default(), &Print::new(true))
            .unwrap();
        assert!(signer.is_none());
    }

    #[test]
    fn auth_signer_uses_sign_with_key() {
        let args = Args {
            sign_with_key: Some(secret_key()),
            ..Default::default()
        };
        let signer = args
            .auth_signer(&locator::Args::default(), &Print::new(true))
            .unwrap()
            .unwrap();
        let expected = ed25519_dalek::SigningKey::from_bytes(&[7; 32]).verifying_key();
        assert_eq!(signer.get_public_key().unwrap().0, expected.to_bytes());
    }

    #[test]
    fn auth_signer_uses_sign_with_ledger() {
        let args = Args {
            sign_with_ledger: true,
            hd_path: Some(3),
            ..Default::default()
        };
        let signer = args
            .auth_signer(&locator::Args::default(), &Print::new(true))
            .unwrap()
            .unwrap();
        assert!(matches!(
            signer.kind,
            SignerKind::Ledger(LedgerEntry { hd_path: 3, .. })
        ));
    }

    #[test]
    fn auth_signer_rejects_sign_with_lab() {
        let args = Args {
            sign_with_lab: true,
            ..Default::default()
        };
        let result = args.auth_signer(&locator::Args::default(), &Print::new(true));
        assert!(matches!(result, Err(Error::LabCannotSignAuthorization)));
    }
}
