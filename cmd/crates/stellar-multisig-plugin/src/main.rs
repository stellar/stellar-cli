use std::collections::HashMap;
use std::io::Read;
use std::process;

use ed25519_dalek::{Signer, SigningKey, VerifyingKey};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use stellar_strkey::Strkey;
use stellar_xdr::curr::{
    DecoratedSignature, Hash, HashIdPreimage, HashIdPreimageSorobanAuthorization, Limits,
    ReadXdr as _, ScMap, ScSymbol, ScVal, Signature, SignatureHint, SorobanAuthorizedInvocation,
    TransactionEnvelope, TransactionSignaturePayload,
    TransactionSignaturePayloadTaggedTransaction, WriteXdr as _,
};

#[derive(thiserror::Error, Debug)]
enum Error {
    #[error("No signers provided. Supply via --plugin-arg multisig:signers=S...,S... or set STELLAR_MULTISIG_SIGNERS")]
    NoSigners,
    #[error("Unknown mode: {0}. Expected 'sign_auth' or 'sign_tx'")]
    UnknownMode(String),
    #[error(
        "Payload validation failed: recomputed hash {computed} does not match provided {provided}"
    )]
    PayloadMismatch { computed: String, provided: String },
    #[error("Invalid secret key '{key}': {details}")]
    InvalidSecretKey { key: String, details: String },
    #[error(transparent)]
    Xdr(#[from] stellar_xdr::curr::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Hex(#[from] hex::FromHexError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Ed25519(#[from] ed25519_dalek::SignatureError),
    #[error(transparent)]
    Strkey(#[from] stellar_strkey::DecodeError),
}

#[derive(Deserialize)]
struct PluginInput {
    mode: String,
    #[serde(default)]
    args: HashMap<String, String>,

    // sign_auth fields
    #[serde(default)]
    payload: Option<String>,
    #[serde(default)]
    network_passphrase: Option<String>,
    #[serde(default)]
    nonce: Option<i64>,
    #[serde(default)]
    signature_expiration_ledger: Option<u32>,
    #[serde(default)]
    root_invocation: Option<String>,

    // sign_tx fields
    #[serde(default)]
    tx_env_xdr: Option<String>,
    #[serde(default)]
    tx_hash: Option<String>,
}

fn main() {
    if let Err(e) = run() {
        eprintln!("stellar-signer-multisig: {e}");
        process::exit(1);
    }
}

fn run() -> Result<(), Error> {
    let mut input_buf = String::new();
    std::io::stdin().read_to_string(&mut input_buf)?;
    let input: PluginInput = serde_json::from_str(&input_buf)?;

    let signing_keys = resolve_signers(&input.args)?;

    match input.mode.as_str() {
        "sign_auth" => handle_sign_auth(&input, &signing_keys),
        "sign_tx" => handle_sign_tx(&input, &signing_keys),
        other => Err(Error::UnknownMode(other.to_string())),
    }
}

/// Resolve signing keys from plugin args or environment variable.
///
/// Checks `args["signers"]` first, then `STELLAR_MULTISIG_SIGNERS` env var.
/// The value is a comma-separated list of Stellar secret keys (S...).
fn resolve_signers(args: &HashMap<String, String>) -> Result<Vec<SigningKey>, Error> {
    let signers_str = args
        .get("signers")
        .cloned()
        .or_else(|| std::env::var("STELLAR_MULTISIG_SIGNERS").ok())
        .unwrap_or_default();

    if signers_str.is_empty() {
        return Err(Error::NoSigners);
    }

    signers_str
        .split(',')
        .map(|s| {
            let s = s.trim();
            match Strkey::from_string(s) {
                Ok(Strkey::PrivateKeyEd25519(secret)) => Ok(SigningKey::from_bytes(&secret.0)),
                Ok(_) => Err(Error::InvalidSecretKey {
                    key: s.to_string(),
                    details: "Not a secret key (S...). Provide ed25519 secret keys.".to_string(),
                }),
                Err(e) => Err(Error::InvalidSecretKey {
                    key: s.to_string(),
                    details: e.to_string(),
                }),
            }
        })
        .collect()
}

// ── sign_auth ──────────────────────────────────────────────────────────────

fn handle_sign_auth(input: &PluginInput, signing_keys: &[SigningKey]) -> Result<(), Error> {
    let payload_hex = input
        .payload
        .as_deref()
        .expect("sign_auth requires 'payload'");
    let network_passphrase = input
        .network_passphrase
        .as_deref()
        .expect("sign_auth requires 'network_passphrase'");
    let nonce = input.nonce.expect("sign_auth requires 'nonce'");
    let signature_expiration_ledger = input
        .signature_expiration_ledger
        .expect("sign_auth requires 'signature_expiration_ledger'");
    let root_invocation_b64 = input
        .root_invocation
        .as_deref()
        .expect("sign_auth requires 'root_invocation'");

    // Decode root invocation from base64 XDR
    let root_invocation =
        SorobanAuthorizedInvocation::from_xdr_base64(root_invocation_b64, Limits::none())?;

    // Recompute the preimage hash and validate against the provided payload
    let network_id = Hash(Sha256::digest(network_passphrase.as_bytes()).into());
    let preimage = HashIdPreimage::SorobanAuthorization(HashIdPreimageSorobanAuthorization {
        network_id,
        invocation: root_invocation,
        nonce,
        signature_expiration_ledger,
    })
    .to_xdr(Limits::none())?;

    let computed_hash = Sha256::digest(preimage);
    let computed_hex = hex::encode(computed_hash);

    if computed_hex != payload_hex {
        return Err(Error::PayloadMismatch {
            computed: computed_hex,
            provided: payload_hex.to_string(),
        });
    }

    let payload: [u8; 32] = computed_hash.into();

    // Sign with each key and build the ScVal::Vec of Map({public_key, signature})
    let mut sig_maps: Vec<ScVal> = Vec::with_capacity(signing_keys.len());
    for key in signing_keys {
        let sig = key.sign(&payload);
        let verifying_key = key.verifying_key();

        let map = ScMap::sorted_from(vec![
            (
                ScVal::Symbol(ScSymbol("public_key".try_into()?)),
                ScVal::Bytes(
                    verifying_key
                        .to_bytes()
                        .to_vec()
                        .try_into()
                        .map_err(stellar_xdr::curr::Error::from)?,
                ),
            ),
            (
                ScVal::Symbol(ScSymbol("signature".try_into()?)),
                ScVal::Bytes(
                    sig.to_bytes()
                        .to_vec()
                        .try_into()
                        .map_err(stellar_xdr::curr::Error::from)?,
                ),
            ),
        ])
        .map_err(stellar_xdr::curr::Error::from)?;
        sig_maps.push(ScVal::Map(Some(map)));
    }

    let result = ScVal::Vec(Some(
        sig_maps
            .try_into()
            .map_err(stellar_xdr::curr::Error::from)?,
    ));

    // Write base64 XDR ScVal to stdout
    let output = result.to_xdr_base64(Limits::none())?;
    print!("{output}");
    Ok(())
}

// ── sign_tx ────────────────────────────────────────────────────────────────

fn handle_sign_tx(input: &PluginInput, signing_keys: &[SigningKey]) -> Result<(), Error> {
    let tx_env_xdr_b64 = input
        .tx_env_xdr
        .as_deref()
        .expect("sign_tx requires 'tx_env_xdr'");
    let tx_hash_hex = input
        .tx_hash
        .as_deref()
        .expect("sign_tx requires 'tx_hash'");
    let network_passphrase = input
        .network_passphrase
        .as_deref()
        .expect("sign_tx requires 'network_passphrase'");

    // Decode the TransactionEnvelope and extract the inner transaction
    let tx_env = TransactionEnvelope::from_xdr_base64(tx_env_xdr_b64, Limits::none())?;

    // Build the correct TransactionSignaturePayload based on envelope type
    let network_id = Hash(Sha256::digest(network_passphrase.as_bytes()).into());
    let tagged_transaction = match &tx_env {
        TransactionEnvelope::Tx(v1) => {
            TransactionSignaturePayloadTaggedTransaction::Tx(v1.tx.clone())
        }
        TransactionEnvelope::TxFeeBump(fb) => {
            TransactionSignaturePayloadTaggedTransaction::TxFeeBump(fb.tx.clone())
        }
        TransactionEnvelope::TxV0(_) => {
            return Err(Error::UnknownMode(
                "V0 transaction envelopes are not supported".to_string(),
            ));
        }
    };
    let sig_payload = TransactionSignaturePayload {
        network_id,
        tagged_transaction,
    };
    let preimage = sig_payload.to_xdr(Limits::none())?;
    let computed_hash = Sha256::digest(preimage);
    let computed_hex = hex::encode(computed_hash);

    if computed_hex != tx_hash_hex {
        return Err(Error::PayloadMismatch {
            computed: computed_hex,
            provided: tx_hash_hex.to_string(),
        });
    }

    let hash_bytes: [u8; 32] = computed_hash.into();

    // Sign with each key and build DecoratedSignature entries
    let mut sigs: Vec<String> = Vec::with_capacity(signing_keys.len());
    for key in signing_keys {
        let sig = key.sign(&hash_bytes);
        let decorated = build_decorated_signature(&key.verifying_key(), &sig);
        let b64 = decorated.to_xdr_base64(Limits::none())?;
        sigs.push(b64);
    }

    // Write JSON array of base64 XDR DecoratedSignature strings to stdout
    let output = serde_json::to_string(&sigs)?;
    print!("{output}");
    Ok(())
}

fn build_decorated_signature(
    verifying_key: &VerifyingKey,
    signature: &ed25519_dalek::Signature,
) -> DecoratedSignature {
    let key_bytes = verifying_key.to_bytes();
    let hint = SignatureHint(key_bytes[28..32].try_into().expect("4 bytes"));
    let sig = Signature(signature.to_bytes().to_vec().try_into().expect("64 bytes"));
    DecoratedSignature {
        hint,
        signature: sig,
    }
}
