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

    /// Explicit signature expiration ledger sequence for Soroban authorization signatures. If not set, defaults to latest ledger + 60 (~5 minutes).
    #[arg(
        long,
        env = "STELLAR_SIGNATURE_EXPIRATION_LEDGER",
        help_heading = HEADING_SIGNING
    )]
    pub signature_expiration_ledger: Option<u32>,
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
    use clap::Parser;

    #[derive(Parser, Debug)]
    struct TestCli {
        #[command(flatten)]
        sign_with: Args,
    }

    #[test]
    fn test_signature_expiration_ledger_arg_parsing() {
        // 1. Defaults to None
        let parsed = TestCli::try_parse_from(["test"]).unwrap();
        assert_eq!(parsed.sign_with.signature_expiration_ledger, None);

        // 2. Parsed from command-line argument
        let parsed = TestCli::try_parse_from(["test", "--signature-expiration-ledger", "123456"]).unwrap();
        assert_eq!(parsed.sign_with.signature_expiration_ledger, Some(123456));

        // 3. Parsed from environment variable
        let env_key = "STELLAR_SIGNATURE_EXPIRATION_LEDGER";
        let prev = std::env::var(env_key).ok();
        std::env::set_var(env_key, "654321");
        let parsed = TestCli::try_parse_from(["test"]).unwrap();
        assert_eq!(parsed.sign_with.signature_expiration_ledger, Some(654321));

        // Command line argument overrides env var
        let parsed = TestCli::try_parse_from(["test", "--signature-expiration-ledger", "789012"]).unwrap();
        assert_eq!(parsed.sign_with.signature_expiration_ledger, Some(789012));

        if let Some(p) = prev {
            std::env::set_var(env_key, p);
        } else {
            std::env::remove_var(env_key);
        }
    }
}
