use std::collections::HashMap;
use std::io::Write as _;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use crate::{
    utils::fee_bump_transaction_hash,
    xdr::{
        self, AccountId, DecoratedSignature, FeeBumpTransactionEnvelope, Hash, HashIdPreimage,
        HashIdPreimageSorobanAuthorization, Limits, Operation, OperationBody, PublicKey, ReadXdr,
        ScAddress, ScMap, ScSymbol, ScVal, Signature, SignatureHint, SorobanAddressCredentials,
        SorobanAuthorizationEntry, SorobanCredentials, Transaction, TransactionEnvelope,
        TransactionV1Envelope, Uint256, VecM, WriteXdr,
    },
};
use ed25519_dalek::{ed25519::signature::Signer as _, Signature as Ed25519Signature};
use sha2::{Digest, Sha256};
use which::which;

use crate::{config::network::Network, print::Print, utils::transaction_hash};

pub mod ledger;

#[cfg(feature = "additional-libs")]
mod keyring;
pub mod secure_store;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Contract addresses are not supported to sign auth entries {address}")]
    ContractAddressAreNotSupported { address: String },
    #[error(transparent)]
    Ed25519(#[from] ed25519_dalek::SignatureError),
    #[error("Missing signing key for account {address}")]
    MissingSignerForAddress { address: String },
    #[error(transparent)]
    TryFromSlice(#[from] std::array::TryFromSliceError),
    #[error("User cancelled signing, perhaps need to add -y")]
    UserCancelledSigning,
    #[error(transparent)]
    Xdr(#[from] xdr::Error),
    #[error("Transaction envelope type not supported")]
    UnsupportedTransactionEnvelopeType,
    #[error(transparent)]
    Url(#[from] url::ParseError),
    #[error(transparent)]
    Open(#[from] std::io::Error),
    #[error("Returning a signature from Lab is not yet supported; Transaction can be found and submitted in lab")]
    ReturningSignatureFromLab,
    #[error(transparent)]
    SecureStore(#[from] secure_store::Error),
    #[error(transparent)]
    Ledger(#[from] ledger::Error),
    #[error(transparent)]
    Decode(#[from] stellar_strkey::DecodeError),
    #[error("Signing plugin '{name}' not found on PATH. Expected binary 'stellar-signer-{name}'")]
    PluginNotFound { name: String },
    #[error("Signing plugin '{name}' failed with exit code {code}")]
    PluginFailed { name: String, code: i32 },
    #[error("Signing plugin '{name}' returned invalid output: {details}")]
    PluginInvalidOutput { name: String, details: String },
    #[error("Signing plugin '{name}' error: {details}")]
    PluginError { name: String, details: String },
    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),
}

/// Convert an `ScAddress` to a Stellar strkey string for plugin signer lookup.
fn sc_address_to_string(address: &ScAddress) -> Result<String, Error> {
    match address {
        ScAddress::Account(AccountId(PublicKey::PublicKeyTypeEd25519(Uint256(bytes)))) => Ok(
            stellar_strkey::Strkey::PublicKeyEd25519(stellar_strkey::ed25519::PublicKey(*bytes))
                .to_string(),
        ),
        ScAddress::Contract(stellar_xdr::curr::ContractId(Hash(bytes))) => {
            Ok(stellar_strkey::Strkey::Contract(stellar_strkey::Contract(*bytes)).to_string())
        }
        ScAddress::MuxedAccount(muxed) => Ok(muxed.to_string()),
        _ => Err(Error::MissingSignerForAddress {
            address: format!("{address:?}"),
        }),
    }
}

/// Use the given signers to sign all `SorobanAuthorizationEntry`s in the given transaction.
///
/// Signers are checked in this order for each auth entry:
/// 1. Plugin signers (matched by address string)
/// 2. Local signers (matched by ed25519 public key bytes)
///
/// Plugin signers can handle any address type (G.../C.../M...).
///
/// If no `SorobanAuthorizationEntry`s need signing (including if none exist), return `Ok(None)`.
///
/// If a `SorobanAuthorizationEntry` needs signing, but a signature cannot be produced for it,
/// return an Error.
pub fn sign_soroban_authorizations(
    raw: &Transaction,
    signers: &[Signer],
    plugin_signers: &[PluginSigner],
    signature_expiration_ledger: u32,
    network_passphrase: &str,
) -> Result<Option<Transaction>, Error> {
    // Check if we have exactly one operation and it's InvokeHostFunction
    let [op @ Operation {
        body: OperationBody::InvokeHostFunction(body),
        ..
    }] = raw.operations.as_slice()
    else {
        return Ok(None);
    };

    let network_id = Hash(Sha256::digest(network_passphrase.as_bytes()).into());

    let mut signed_auths = Vec::with_capacity(body.auth.len());
    for raw_auth in body.auth.as_slice() {
        let auth = raw_auth.clone();
        let SorobanAuthorizationEntry {
            credentials: SorobanCredentials::Address(ref credentials),
            ..
        } = auth
        else {
            // Doesn't need special signing (e.g., source account credentials)
            signed_auths.push(auth);
            continue;
        };
        let SorobanAddressCredentials { ref address, .. } = credentials;

        let needle: &[u8; 32] = match address {
            ScAddress::Account(AccountId(PublicKey::PublicKeyTypeEd25519(Uint256(ref a)))) => a,
            // Non-account addresses without a plugin cannot be signed locally
            other => {
                return Err(Error::MissingSignerForAddress {
                    address: sc_address_to_string(other).unwrap_or_else(|_| format!("{other:?}")),
                });
            }
        };

        let plugin_signer: Signer;
        let mut signer: Option<&Signer> = None;
        // 1. Check for a plugin signer mapped to this address
        // 2. If no plugin signer, check for a local signer with a matching public key
        if let Some(plugin) = plugin_signers.iter().find(|p| p.sc_address == *address) {
            plugin_signer = Signer {
                kind: SignerKind::Plugin(plugin.clone()),
                print: Print::new(false),
            };
            signer = Some(&plugin_signer);
        } else if let Some(s) = signers.iter().find(|s| {
            if let Ok(pk) = s.get_public_key() {
                pk.0 == *needle
            } else {
                false
            }
        }) {
            signer = Some(s);
        }

        match signer {
            Some(signer) => {
                let signed_entry = sign_soroban_authorization_entry(
                    raw_auth,
                    signer,
                    signature_expiration_ledger,
                    &network_id,
                    network_passphrase,
                )?;
                signed_auths.push(signed_entry);
            }
            None => {
                return Err(Error::MissingSignerForAddress {
                    address: stellar_strkey::Strkey::PublicKeyEd25519(
                        stellar_strkey::ed25519::PublicKey(*needle),
                    )
                    .to_string(),
                });
            }
        }
    }

    // No signatures were made, return None to indicate no change to the transaction
    if signed_auths.is_empty() {
        return Ok(None);
    }

    // Build updated transaction with signed auth entries
    let mut updated_op = op.clone();
    if let OperationBody::InvokeHostFunction(ref mut updated_body) = updated_op.body {
        let mut tx = raw.clone();
        updated_body.auth = signed_auths.try_into()?;
        tx.operations = vec![updated_op].try_into()?;
        Ok(Some(tx))
    } else {
        Ok(None)
    }
}

fn sign_soroban_authorization_entry(
    raw: &SorobanAuthorizationEntry,
    signer: &Signer,
    signature_expiration_ledger: u32,
    network_id: &Hash,
    network_passphrase: &str,
) -> Result<SorobanAuthorizationEntry, Error> {
    let mut auth = raw.clone();
    let SorobanAuthorizationEntry {
        credentials: SorobanCredentials::Address(ref mut credentials),
        ..
    } = auth
    else {
        // Doesn't need special signing
        return Ok(auth);
    };
    let SorobanAddressCredentials { nonce, .. } = credentials;

    let signature_scval = signer.sign_auth_entry(
        &auth.root_invocation,
        *nonce,
        signature_expiration_ledger,
        network_id.clone(),
        network_passphrase,
    )?;
    credentials.signature = signature_scval;
    credentials.signature_expiration_ledger = signature_expiration_ledger;
    auth.credentials = SorobanCredentials::Address(credentials.clone());
    Ok(auth)
}

pub struct Signer {
    pub kind: SignerKind,
    pub print: Print,
}

#[allow(clippy::module_name_repetitions, clippy::large_enum_variant)]
pub enum SignerKind {
    Local(LocalKey),
    Ledger(ledger::LedgerType),
    Lab,
    SecureStore(SecureStoreEntry),
    Plugin(PluginSigner),
}

// It is advised to use the sign_with module, which handles creating a Signer with the appropriate SignerKind
impl Signer {
    pub async fn sign_tx(
        &self,
        tx: Transaction,
        network: &Network,
    ) -> Result<TransactionEnvelope, Error> {
        let tx_env = TransactionEnvelope::Tx(TransactionV1Envelope {
            tx,
            signatures: VecM::default(),
        });
        self.sign_tx_env(&tx_env, network).await
    }

    pub async fn sign_tx_env(
        &self,
        tx_env: &TransactionEnvelope,
        network: &Network,
    ) -> Result<TransactionEnvelope, Error> {
        match &tx_env {
            TransactionEnvelope::Tx(TransactionV1Envelope { tx, signatures }) => {
                let tx_hash = transaction_hash(tx, &network.network_passphrase)?;
                self.print
                    .infoln(format!("Signing transaction: {}", hex::encode(tx_hash),));
                let decorated_signature = self.sign_tx_hash(tx_hash, tx_env, network).await?;
                let mut sigs = signatures.clone().into_vec();
                sigs.extend(decorated_signature);
                Ok(TransactionEnvelope::Tx(TransactionV1Envelope {
                    tx: tx.clone(),
                    signatures: sigs.try_into()?,
                }))
            }
            TransactionEnvelope::TxFeeBump(FeeBumpTransactionEnvelope { tx, signatures }) => {
                let tx_hash = fee_bump_transaction_hash(tx, &network.network_passphrase)?;
                self.print.infoln(format!(
                    "Signing fee bump transaction: {}",
                    hex::encode(tx_hash),
                ));
                let decorated_signature = self.sign_tx_hash(tx_hash, tx_env, network).await?;
                let mut sigs = signatures.clone().into_vec();
                sigs.extend(decorated_signature);
                Ok(TransactionEnvelope::TxFeeBump(FeeBumpTransactionEnvelope {
                    tx: tx.clone(),
                    signatures: sigs.try_into()?,
                }))
            }
            TransactionEnvelope::TxV0(_) => Err(Error::UnsupportedTransactionEnvelopeType),
        }
    }

    // when we implement this for ledger we'll need it to be async so we can await for the ledger's public key
    pub fn get_public_key(&self) -> Result<stellar_strkey::ed25519::PublicKey, Error> {
        match &self.kind {
            SignerKind::Local(local_key) => Ok(stellar_strkey::ed25519::PublicKey::from_payload(
                local_key.key.verifying_key().as_bytes(),
            )?),
            SignerKind::Ledger(_ledger) => todo!("ledger device is not implemented"),
            SignerKind::Lab => Err(Error::ReturningSignatureFromLab),
            SignerKind::SecureStore(secure_store_entry) => Ok(secure_store_entry.public_key),
            SignerKind::Plugin(_) => Err(Error::PluginError {
                name: "plugin".to_string(),
                details: "Plugins do not expose a public key directly".to_string(),
            }),
        }
    }

    pub fn get_sc_address(&self) -> Result<ScAddress, Error> {
        match &self.kind {
            SignerKind::Local(_) | SignerKind::SecureStore(_) => {
                let pk = self.get_public_key()?;
                Ok(ScAddress::Account(AccountId(
                    PublicKey::PublicKeyTypeEd25519(Uint256(pk.0)),
                )))
            }
            SignerKind::Ledger(_ledger) => todo!("ledger device is not implemented"),
            SignerKind::Lab => Err(Error::ReturningSignatureFromLab),
            SignerKind::Plugin(plugin_signer) => Ok(plugin_signer.sc_address.clone()),
        }
    }

    // when we implement this for ledger we'll need it to be async so we can await the user approved the tx on the ledger device
    pub fn sign_payload(&self, payload: [u8; 32]) -> Result<Ed25519Signature, Error> {
        match &self.kind {
            SignerKind::Local(local_key) => local_key.sign_payload(payload),
            SignerKind::Ledger(_ledger) => todo!("ledger device is not implemented"),
            SignerKind::Lab => Err(Error::ReturningSignatureFromLab),
            SignerKind::SecureStore(secure_store_entry) => secure_store_entry.sign_payload(payload),
            SignerKind::Plugin(plugin_signer) => Err(Error::PluginError {
                name: plugin_signer.name.clone(),
                details: "sign payload is not supported".to_string(),
            }),
        }
    }

    pub fn sign_auth_entry(
        &self,
        root_invocation: &xdr::SorobanAuthorizedInvocation,
        nonce: i64,
        signature_expiration_ledger: u32,
        network_id: Hash,
        network_passphrase: &str,
    ) -> Result<ScVal, Error> {
        let preimage = HashIdPreimage::SorobanAuthorization(HashIdPreimageSorobanAuthorization {
            network_id: network_id.clone(),
            invocation: root_invocation.clone(),
            nonce,
            signature_expiration_ledger,
        })
        .to_xdr(Limits::none())?;

        let payload = Sha256::digest(preimage);
        let p: [u8; 32] = payload.as_slice().try_into()?;

        if let SignerKind::Plugin(plugin_signer) = &self.kind {
            plugin_signer.sign_auth_entry(
                p,
                root_invocation,
                nonce,
                signature_expiration_ledger,
                network_passphrase,
            )
        } else {
            // for local signers, sign the payload directly and build the ScVal signature
            let signature = self.sign_payload(p)?;
            let public_key_vec = self.get_public_key()?.0.to_vec();

            let map = ScMap::sorted_from(vec![
                (
                    ScVal::Symbol(ScSymbol("public_key".try_into()?)),
                    ScVal::Bytes(public_key_vec.try_into().map_err(Error::Xdr)?),
                ),
                (
                    ScVal::Symbol(ScSymbol("signature".try_into()?)),
                    ScVal::Bytes(
                        signature
                            .to_bytes()
                            .to_vec()
                            .try_into()
                            .map_err(Error::Xdr)?,
                    ),
                ),
            ])
            .map_err(Error::Xdr)?;
            Ok(ScVal::Vec(Some(
                vec![ScVal::Map(Some(map))].try_into().map_err(Error::Xdr)?,
            )))
        }
    }

    async fn sign_tx_hash(
        &self,
        tx_hash: [u8; 32],
        tx_env: &TransactionEnvelope,
        network: &Network,
    ) -> Result<Vec<DecoratedSignature>, Error> {
        match &self.kind {
            SignerKind::Local(key) => key.sign_tx_hash(tx_hash),
            SignerKind::Lab => Lab::sign_tx_env(tx_env, network, &self.print),
            SignerKind::Ledger(ledger) => ledger
                .sign_transaction_hash(&tx_hash)
                .await
                .map_err(Error::from),
            SignerKind::SecureStore(entry) => entry.sign_tx_hash(tx_hash),
            SignerKind::Plugin(plugin) => {
                plugin.sign_tx_hash(tx_env, tx_hash, &network.network_passphrase)
            }
        }
    }
}

pub struct LocalKey {
    pub key: ed25519_dalek::SigningKey,
}

impl LocalKey {
    pub fn sign_tx_hash(&self, tx_hash: [u8; 32]) -> Result<Vec<DecoratedSignature>, Error> {
        let hint = SignatureHint(self.key.verifying_key().to_bytes()[28..].try_into()?);
        let signature = Signature(self.key.sign(&tx_hash).to_bytes().to_vec().try_into()?);
        Ok(vec![DecoratedSignature { hint, signature }])
    }

    pub fn sign_payload(&self, payload: [u8; 32]) -> Result<Ed25519Signature, Error> {
        Ok(self.key.sign(&payload))
    }
}

pub struct Lab;

impl Lab {
    const URL: &str = "https://lab.stellar.org/transaction/cli-sign";

    pub fn sign_tx_env(
        tx_env: &TransactionEnvelope,
        network: &Network,
        printer: &Print,
    ) -> Result<Vec<DecoratedSignature>, Error> {
        let xdr = tx_env.to_xdr_base64(Limits::none())?;

        let mut url = url::Url::parse(Self::URL)?;
        url.query_pairs_mut()
            .append_pair("networkPassphrase", &network.network_passphrase)
            .append_pair("xdr", &xdr);
        let url = url.to_string();

        printer.globeln(format!("Opening lab to sign transaction: {url}"));
        open::that(url)?;

        Err(Error::ReturningSignatureFromLab)
    }
}

pub struct SecureStoreEntry {
    pub name: String,
    pub hd_path: Option<usize>,
    pub public_key: stellar_strkey::ed25519::PublicKey,
}

impl SecureStoreEntry {
    pub fn new(name: String, hd_path: Option<usize>) -> Result<Self, Error> {
        let public_key = secure_store::get_public_key(&name, hd_path)?;
        Ok(Self {
            name,
            hd_path,
            public_key,
        })
    }

    pub fn sign_tx_hash(&self, tx_hash: [u8; 32]) -> Result<Vec<DecoratedSignature>, Error> {
        let hint = SignatureHint(self.public_key.0[28..].try_into()?);

        let signed_tx_hash = secure_store::sign_tx_data(&self.name, self.hd_path, &tx_hash)?;

        let signature = Signature(signed_tx_hash.clone().try_into()?);
        Ok(vec![DecoratedSignature { hint, signature }])
    }

    pub fn sign_payload(&self, payload: [u8; 32]) -> Result<Ed25519Signature, Error> {
        let signed_bytes = secure_store::sign_tx_data(&self.name, self.hd_path, &payload)?;
        let sig = Ed25519Signature::from_bytes(signed_bytes.as_slice().try_into()?);
        Ok(sig)
    }
}

/// A signing plugin that delegates signing to an external binary.
///
/// The plugin binary is discovered on `$PATH` as `stellar-signer-{name}`.
/// It communicates via JSON on stdin/stdout:
///
/// **Auth entry signing** (`sign_auth` mode):
/// - Input: `{ "mode": "sign_auth", "network_passphrase", "address", "nonce",
///  "signature_expiration_ledger", "root_invocation" (base64 XDR), "args": {...} }`
/// - Output: `Base64 XDR string` of the ScVal representing the signature credential for the auth entry
///
/// **Transaction signing** (`sign_tx` mode):
/// - Input: `{ "mode": "sign_tx", "network_passphrase", "tx_env_xdr" (base64 XDR TransactionEnvelope), "tx_hash", "args": {...} }`
/// - Output: `JSON array of base64 XDR strings` representing DecoratedSignatures to add to the transaction envelope
///
/// The plugin's stderr is inherited so it can print prompts, progress, or open browsers.
#[derive(Clone)]
pub struct PluginSigner {
    /// Name of the plugin (resolved to binary `stellar-signer-{name}` on PATH).
    pub name: String,
    /// The resolved path to the plugin binary.
    pub bin_path: PathBuf,
    /// The Stellar address this plugin is mapped to (G.../C.../M...).
    pub sc_address: ScAddress,
    /// Extra key-value arguments forwarded to the plugin in the `args` JSON field.
    pub args: HashMap<String, String>,
}

impl PluginSigner {
    /// Create a new `PluginSigner` by resolving the plugin binary on `$PATH`.
    pub fn new(
        name: &str,
        sc_address: ScAddress,
        args: HashMap<String, String>,
    ) -> Result<Self, Error> {
        let bin_path = find_signer_plugin_bin(name)?;

        Ok(Self {
            name: name.to_string(),
            bin_path,
            sc_address,
            args,
        })
    }

    /// Sign a single `SorobanAuthorizationEntry` by invoking the plugin in `sign_auth` mode.
    ///
    /// The plugin receives the auth entry context as JSON and returns the `ScVal` credential
    /// signature. The CLI handles all transaction envelope parsing and reassembly.
    pub fn sign_auth_entry(
        &self,
        payload: [u8; 32],
        root_invocation: &xdr::SorobanAuthorizedInvocation,
        nonce: i64,
        signature_expiration_ledger: u32,
        network_passphrase: &str,
    ) -> Result<ScVal, Error> {
        let input = serde_json::json!({
            "mode": "sign_auth",
            "payload": hex::encode(payload),
            "network_passphrase": network_passphrase,
            "address": self.sc_address.to_xdr_base64(Limits::none())?,
            "nonce": nonce,
            "signature_expiration_ledger": signature_expiration_ledger,
            "root_invocation": root_invocation.to_xdr_base64(Limits::none())?,
            "args": self.args,
        });

        let output = self.invoke_plugin(&input)?;

        // Decode the ScVal from base64 XDR
        ScVal::from_xdr_base64(output.trim_ascii_end(), Limits::none()).map_err(|e| {
            Error::PluginInvalidOutput {
                name: self.name.clone(),
                details: format!("Failed to decode ScVal from base64 XDR: {e}"),
            }
        })
    }

    /// Sign a transaction hash by invoking the plugin in `sign_tx` mode.
    ///
    /// Returns a `Vec<DecoratedSignature>` for inclusion in the transaction envelope.
    pub fn sign_tx_hash(
        &self,
        tx_env: &TransactionEnvelope,
        tx_hash: [u8; 32],
        network_passphrase: &str,
    ) -> Result<Vec<DecoratedSignature>, Error> {
        let tx_env_xdr = tx_env.to_xdr_base64(Limits::none())?;
        let input = serde_json::json!({
            "mode": "sign_tx",
            "tx_env_xdr": tx_env_xdr,
            "tx_hash": hex::encode(tx_hash),
            "network_passphrase": network_passphrase,
            "args": self.args,
        });

        let output = self.invoke_plugin(&input)?;
        let output_str = std::str::from_utf8(&output).map_err(|e| Error::PluginInvalidOutput {
            name: self.name.clone(),
            details: format!("Plugin output is not valid UTF-8: {e}"),
        })?;
        let sig_strings: Vec<String> =
            serde_json::from_str(output_str).map_err(|e| Error::PluginInvalidOutput {
                name: self.name.clone(),
                details: format!(
                    "Expected JSON array of base64 XDR DecoratedSignature strings: {e}"
                ),
            })?;
        sig_strings
            .iter()
            .map(|s| {
                DecoratedSignature::from_xdr_base64(s.trim(), Limits::none()).map_err(|e| {
                    Error::PluginInvalidOutput {
                        name: self.name.clone(),
                        details: format!(
                            "Failed to decode DecoratedSignature from base64 XDR: {e}"
                        ),
                    }
                })
            })
            .collect()
    }

    /// Spawn the plugin process, write JSON to its stdin, and read the response from stdout.
    /// The plugin's stderr is inherited so it can print prompts or progress to the terminal.
    fn invoke_plugin(&self, input: &serde_json::Value) -> Result<Vec<u8>, Error> {
        let mut child = Command::new(&self.bin_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit()) // Plugin can print prompts, open browsers, etc.
            .spawn()
            .map_err(|e| Error::PluginError {
                name: self.name.clone(),
                details: format!("Failed to spawn plugin: {e}"),
            })?;

        // Write JSON input to stdin
        if let Some(mut stdin) = child.stdin.take() {
            let input_bytes = serde_json::to_vec(input)?;
            stdin
                .write_all(&input_bytes)
                .map_err(|e| Error::PluginError {
                    name: self.name.clone(),
                    details: format!("Failed to write to plugin stdin: {e}"),
                })?;
            // stdin is dropped here, closing the pipe
        }

        // Wait for the plugin to finish and read stdout
        let output = child.wait_with_output().map_err(|e| Error::PluginError {
            name: self.name.clone(),
            details: format!("Failed to wait for plugin: {e}"),
        })?;

        if !output.status.success() {
            let code = output.status.code().unwrap_or(-1);
            return Err(Error::PluginFailed {
                name: self.name.clone(),
                code,
            });
        }

        Ok(output.stdout)
    }
}

/// Find a signer plugin binary on `$PATH` by name.
/// Looks for `stellar-signer-{name}` first, then `soroban-signer-{name}`.
fn find_signer_plugin_bin(name: &str) -> Result<PathBuf, Error> {
    if let Ok(path) = which(format!("stellar-signer-{name}")) {
        Ok(path)
    } else if let Ok(path) = which(format!("soroban-signer-{name}")) {
        Ok(path)
    } else {
        Err(Error::PluginNotFound {
            name: name.to_string(),
        })
    }
}
