use std::{collections::HashMap, str::FromStr};

use crate::{
    config::{UnresolvedMuxedAccount, UnresolvedScAddress},
    print::Print,
    signer::{self, ledger, PluginSigner, Signer, SignerKind},
    xdr::{self, TransactionEnvelope},
};

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
    #[error("Invalid --sign-with-plugin format '{value}'. Expected 'plugin-name=address'.")]
    InvalidPluginArg { value: String },
    #[error("Invalid --plugin-arg format '{value}'. Expected 'plugin-name:key=value'.")]
    InvalidPluginArgExtra { value: String },
    #[error(transparent)]
    UnresolvedScAddress(#[from] crate::config::sc_address::Error),
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

    /// Sign auth entries with an external signing plugin. Format: `plugin-name=address`.
    /// Maps the given address (G.../C.../M.../alias) to a plugin binary to execute signing (`stellar-signer-{plugin-name}` on PATH).
    /// Can be specified multiple times to map different addresses to different plugins.
    /// Example: `--sign-with-plugin multisig=CDLDY...` uses `stellar-signer-multisig` for that address.
    #[arg(long, num_args = 1)]
    pub sign_with_plugin: Vec<String>,

    /// Pass extra arguments to a signing plugin. Format: `plugin-name:key=value`.
    /// These are forwarded to the plugin in the `args` JSON field.
    /// Can be specified multiple times. It is recommended to pass sensitive values via environment
    /// variables within the plugin instead.
    /// Example: `--plugin-arg multisig:signer_1=S...`
    #[arg(long, num_args = 1)]
    pub plugin_arg: Vec<String>,
}

impl Args {
    /// Parse `--sign-with-plugin` and `--plugin-arg` flags into a map of address → `PluginSigner`.
    pub fn build_plugin_signers(
        &self,
        locator: &locator::Args,
        network_passphrase: &str,
    ) -> Result<Vec<PluginSigner>, Error> {
        // First, collect plugin-args grouped by plugin name
        let mut plugin_args: HashMap<String, HashMap<String, String>> = HashMap::new();
        for arg in &self.plugin_arg {
            let (plugin_name, kv) = arg
                .split_once(':')
                .ok_or_else(|| Error::InvalidPluginArgExtra { value: arg.clone() })?;
            let (key, value) = kv
                .split_once('=')
                .ok_or_else(|| Error::InvalidPluginArgExtra { value: arg.clone() })?;
            plugin_args
                .entry(plugin_name.to_string())
                .or_default()
                .insert(key.to_string(), value.to_string());
        }

        // Then, build PluginSigner for each --sign-with-plugin entry
        let mut signers: Vec<PluginSigner> = Vec::new();
        for entry in &self.sign_with_plugin {
            let (plugin_name, address) =
                entry
                    .split_once('=')
                    .ok_or_else(|| Error::InvalidPluginArg {
                        value: entry.clone(),
                    })?;
            let unresolved_sc_address = UnresolvedScAddress::from_str(address)?;
            let sc_address = unresolved_sc_address.resolve(locator, network_passphrase)?;
            let args = plugin_args.remove(plugin_name).unwrap_or_default();
            let plugin = PluginSigner::new(plugin_name, sc_address, args)?;
            signers.push(plugin);
        }

        Ok(signers)
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

        // Check if the tx source account matches an account specified in --sign-with-plugin
        // If so, use the corresponding plugin signer to sign the transaction and return early.
        if !self.sign_with_plugin.is_empty() {
            let source_account = match tx {
                TransactionEnvelope::Tx(tx) => &tx.tx.source_account,
                TransactionEnvelope::TxFeeBump(fb) => &fb.tx.fee_source,
                TransactionEnvelope::TxV0(tx) => {
                    &xdr::MuxedAccount::Ed25519(tx.tx.source_account_ed25519.clone())
                }
            };
            let sc_address = match source_account {
                xdr::MuxedAccount::Ed25519(ed25519) => xdr::ScAddress::Account(xdr::AccountId(
                    xdr::PublicKey::PublicKeyTypeEd25519(ed25519.clone()),
                )),
                xdr::MuxedAccount::MuxedEd25519(muxed_ed25519) => {
                    xdr::ScAddress::MuxedAccount(xdr::MuxedEd25519Account {
                        id: muxed_ed25519.id,
                        ed25519: muxed_ed25519.ed25519.clone(),
                    })
                }
            };
            if let Some(plugin_signer) = self
                .build_plugin_signers(locator, &network.network_passphrase)?
                .into_iter()
                .find(|p| p.sc_address == sc_address)
            {
                let signer = Signer {
                    kind: SignerKind::Plugin(plugin_signer),
                    print,
                };
                return Ok(signer.sign_tx_env(tx, network).await?);
            }
        }

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
