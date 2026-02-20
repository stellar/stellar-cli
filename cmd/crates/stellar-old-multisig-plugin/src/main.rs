//! A traditional stellar-cli plugin that signs Soroban auth entries and/or the transaction
//! envelope for multi-sig Stellar accounts.
//!
//! This plugin is discovered as a CLI subcommand `stellar sign-auth-multisig` via the binary
//! name `stellar-sign-auth-multisig` on `$PATH`.
//!
//! ## Usage (auth entry signing)
//!
//! ```bash
//! stellar contract invoke --source feepayer --id $CONTRACT --build-only -- my_fn --arg1 val1 \
//!   | stellar tx simulate \
//!   | stellar sign-auth-multisig \
//!       --signers S...,S... \
//!       --signature-expiration-ledger 12345 \
//!       --network-passphrase "Test SDF Network ; September 2015" \
//!   | stellar tx simulate \
//!   | stellar tx sign --sign-with-key feepayer \
//!   | stellar tx send
//! ```
//!
//! ## Usage (as source account)
//!
//! ```bash
//! stellar contract invoke --id $CONTRACT --build-only -- my_fn --arg1 val1 \
//!   | stellar tx simulate \
//!   | stellar sign-auth-multisig \
//!       --signers S...,S... \
//!       --signature-expiration-ledger 12345 \
//!       --network-passphrase "Test SDF Network ; September 2015" \
//!       --sign-tx \
//!   | stellar tx send
//! ```
//!
//! It reads a base64 `TransactionEnvelope` from stdin, signs all `SorobanAddressCredentials`
//! auth entries with the provided keys (ordered by public key ascending), and optionally signs
//! the transaction envelope itself. The modified envelope is written to stdout.

use std::io::{self, Read};

use clap::Parser;
use ed25519_dalek::{Signer, SigningKey, VerifyingKey};
use sha2::{Digest, Sha256};
use stellar_strkey::Strkey;
use stellar_xdr::curr::{
    DecoratedSignature, Hash, HashIdPreimage, HashIdPreimageSorobanAuthorization, Limited, Limits,
    Operation, OperationBody, ReadXdr, ScMap, ScSymbol, ScVal, Signature, SignatureHint,
    SorobanAddressCredentials, SorobanAuthorizationEntry, SorobanCredentials, TransactionEnvelope,
    TransactionSignaturePayload, TransactionSignaturePayloadTaggedTransaction,
    TransactionV1Envelope, WriteXdr,
};

#[derive(thiserror::Error, Debug)]
enum Error {
    #[error("No signers provided. Use --signers S...,S... or set STELLAR_MULTISIG_SIGNERS")]
    NoSigners,
    #[error("Invalid secret key '{key}': {details}")]
    InvalidSecretKey { key: String, details: String },
    #[error("Unsupported transaction envelope type (V0)")]
    UnsupportedEnvelopeType,
    #[error("No InvokeHostFunction operation found in transaction")]
    NoInvokeHostFunction,
    #[error(transparent)]
    Xdr(#[from] stellar_xdr::curr::Error),
    #[error(transparent)]
    Io(#[from] io::Error),
}

#[derive(Parser, Debug, Clone)]
#[command(
    name = "sign-auth-multisig",
    about = "Sign Soroban auth entries with multiple ed25519 keys for multi-sig accounts"
)]
struct Cli {
    /// Comma-separated list of Stellar secret keys (S...) to sign with.
    /// Can also be set via STELLAR_MULTISIG_SIGNERS env var.
    #[arg(long, env = "STELLAR_MULTISIG_SIGNERS")]
    signers: String,

    /// The ledger sequence at which the auth signatures expire.
    #[arg(long)]
    signature_expiration_ledger: u32,

    /// Network passphrase (defaults to testnet).
    #[arg(long, default_value = "Test SDF Network ; September 2015")]
    network_passphrase: String,

    /// Also sign the transaction envelope itself (for multi-sig source accounts).
    #[arg(long, default_value_t = false)]
    sign_tx: bool,
}

fn main() {
    if let Err(e) = run() {
        eprintln!("stellar-sign-auth-multisig: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let signing_keys = resolve_signers(&cli.signers)?;
    if signing_keys.is_empty() {
        return Err(Error::NoSigners.into());
    }

    // Sort signing keys by public key bytes ascending (required by Stellar auth check)
    let mut sorted_keys: Vec<&SigningKey> = signing_keys.iter().collect();
    sorted_keys.sort_by_key(|k| k.verifying_key().to_bytes());

    let network_id = Hash(Sha256::digest(cli.network_passphrase.as_bytes()).into());

    // Read the transaction envelope from stdin (skip whitespace)
    let mut txe = TransactionEnvelope::read_xdr_base64_to_end(&mut Limited::new(
        SkipWhitespace::new(io::stdin()),
        Limits::none(),
    ))?;

    // Sign auth entries
    sign_auth_entries(
        &mut txe,
        &sorted_keys,
        &network_id,
        cli.signature_expiration_ledger,
    )?;

    // Optionally sign the transaction envelope itself
    if cli.sign_tx {
        sign_transaction_envelope(
            &mut txe,
            &signing_keys, // tx signing doesn't require public key ordering
            &network_id,
        )?;
    }

    // Output the modified transaction envelope to stdout
    println!("{}", txe.to_xdr_base64(Limits::none())?);

    Ok(())
}

/// Parse a comma-separated list of S... secret keys into SigningKeys.
fn resolve_signers(signers_str: &str) -> Result<Vec<SigningKey>, Error> {
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

/// Sign all SorobanAddressCredentials auth entries in the transaction.
fn sign_auth_entries(
    txe: &mut TransactionEnvelope,
    sorted_keys: &[&SigningKey],
    network_id: &Hash,
    signature_expiration_ledger: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    let tx = match txe {
        TransactionEnvelope::Tx(TransactionV1Envelope { ref mut tx, .. }) => tx,
        TransactionEnvelope::TxV0(_) => return Err(Error::UnsupportedEnvelopeType.into()),
        TransactionEnvelope::TxFeeBump(_) => {
            // Fee bump transactions don't contain soroban operations directly
            eprintln!("Warning: Fee bump transactions don't contain auth entries to sign");
            return Ok(());
        }
    };

    // VecM doesn't implement DerefMut, so we need to work with the operations
    // by converting to a Vec, modifying, and converting back.
    let mut ops = tx.operations.to_vec();
    let op = match ops.as_mut_slice() {
        [op @ Operation {
            body: OperationBody::InvokeHostFunction(_),
            ..
        }] => op,
        _ => return Err(Error::NoInvokeHostFunction.into()),
    };

    let OperationBody::InvokeHostFunction(ref mut body) = op.body else {
        return Err(Error::NoInvokeHostFunction.into());
    };

    let mut signed_auths: Vec<SorobanAuthorizationEntry> = Vec::with_capacity(body.auth.len());

    for auth in body.auth.as_slice() {
        let SorobanAuthorizationEntry {
            credentials: SorobanCredentials::Address(ref credentials),
            ..
        } = auth
        else {
            // Not address credentials — pass through unchanged
            signed_auths.push(auth.clone());
            continue;
        };

        let SorobanAddressCredentials { nonce, .. } = credentials;

        eprintln!(
            "Signing auth entry:\n{}",
            serde_json::to_string_pretty(&auth.root_invocation)?
        );

        // Build the payload that the network expects to be signed
        let preimage = HashIdPreimage::SorobanAuthorization(HashIdPreimageSorobanAuthorization {
            network_id: network_id.clone(),
            nonce: *nonce,
            signature_expiration_ledger,
            invocation: auth.root_invocation.clone(),
        })
        .to_xdr(Limits::none())?;

        let payload_hash = Sha256::digest(preimage);
        eprintln!("Payload Hash: {}", hex::encode(payload_hash));

        let payload: [u8; 32] = payload_hash.into();

        // Sign with each key (sorted by public key) and build the credential signature
        let mut sig_maps: Vec<ScVal> = Vec::with_capacity(sorted_keys.len());
        for key in sorted_keys {
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

        let signature_scval = ScVal::Vec(Some(
            sig_maps
                .try_into()
                .map_err(stellar_xdr::curr::Error::from)?,
        ));

        // Reassemble the auth entry with the new signature
        let mut signed_auth = auth.clone();
        if let SorobanCredentials::Address(ref mut creds) = signed_auth.credentials {
            creds.signature_expiration_ledger = signature_expiration_ledger;
            creds.signature = signature_scval;
        }

        eprintln!(
            "Authorized:\n{}",
            serde_json::to_string_pretty(&signed_auth.credentials)?
        );

        signed_auths.push(signed_auth);
    }

    body.auth = signed_auths.try_into()?;
    tx.operations = ops.try_into()?;
    Ok(())
}

/// Sign the transaction envelope itself with all provided keys.
fn sign_transaction_envelope(
    txe: &mut TransactionEnvelope,
    signing_keys: &[SigningKey],
    network_id: &Hash,
) -> Result<(), Box<dyn std::error::Error>> {
    let tagged_transaction = match txe {
        TransactionEnvelope::Tx(TransactionV1Envelope { ref tx, .. }) => {
            TransactionSignaturePayloadTaggedTransaction::Tx(tx.clone())
        }
        TransactionEnvelope::TxFeeBump(ref fb) => {
            TransactionSignaturePayloadTaggedTransaction::TxFeeBump(fb.tx.clone())
        }
        TransactionEnvelope::TxV0(_) => return Err(Error::UnsupportedEnvelopeType.into()),
    };

    let sig_payload = TransactionSignaturePayload {
        network_id: network_id.clone(),
        tagged_transaction,
    };
    let preimage = sig_payload.to_xdr(Limits::none())?;
    let tx_hash = Sha256::digest(preimage);
    let hash_bytes: [u8; 32] = tx_hash.into();

    eprintln!("Signing transaction: {}", hex::encode(hash_bytes));

    let mut new_sigs: Vec<DecoratedSignature> = Vec::with_capacity(signing_keys.len());
    for key in signing_keys {
        let sig = key.sign(&hash_bytes);
        new_sigs.push(build_decorated_signature(&key.verifying_key(), &sig));
    }

    // Append signatures to the envelope
    match txe {
        TransactionEnvelope::Tx(TransactionV1Envelope {
            ref mut signatures, ..
        }) => {
            let mut sigs = signatures.clone().into_vec();
            sigs.extend(new_sigs);
            *signatures = sigs.try_into()?;
        }
        TransactionEnvelope::TxFeeBump(ref mut fb) => {
            let mut sigs = fb.signatures.clone().into_vec();
            sigs.extend(new_sigs);
            fb.signatures = sigs.try_into()?;
        }
        TransactionEnvelope::TxV0(_) => return Err(Error::UnsupportedEnvelopeType.into()),
    }

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

/// A wrapper around a `Read` that strips ASCII whitespace, allowing base64 XDR
/// to be read from stdin even when piped with newlines.
struct SkipWhitespace<R: Read> {
    inner: R,
}

impl<R: Read> SkipWhitespace<R> {
    fn new(inner: R) -> Self {
        SkipWhitespace { inner }
    }
}

impl<R: Read> Read for SkipWhitespace<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.inner.read(buf)?;
        let mut written = 0;
        for read in 0..n {
            if !buf[read].is_ascii_whitespace() {
                buf[written] = buf[read];
                written += 1;
            }
        }
        Ok(written)
    }
}
