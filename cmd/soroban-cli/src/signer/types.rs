use crossterm::event::{read, Event, KeyCode};
use ed25519_dalek::ed25519::signature::Signer;
use sha2::{Digest, Sha256};

use crate::{
    config::network::Network,
    xdr::{
        self, DecoratedSignature, Limits, Signature, SignatureHint, TransactionEnvelope,
        TransactionSignaturePayload, TransactionSignaturePayloadTaggedTransaction,
        TransactionV1Envelope, WriteXdr,
    },
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Contract addresses are not supported to sign auth entries {address}")]
    ContractAddressAreNotSupported { address: String },
    #[error(transparent)]
    Ed25519(#[from] ed25519_dalek::SignatureError),
    #[error("Missing signing key for account {address}")]
    MissingSignerForAddress { address: String },
    #[error(transparent)]
    Xdr(#[from] xdr::Error),
    #[error(transparent)]
    Rpc(#[from] crate::rpc::Error),
    #[error("User cancelled signing, perhaps need to remove --check")]
    UserCancelledSigning,
    #[error("Only Transaction envelope V1 type is supported")]
    UnsupportedTransactionEnvelopeType,
}

/// Calculate the hash of a Transaction
pub fn transaction_hash(
    txn: &xdr::Transaction,
    network_passphrase: &str,
) -> Result<[u8; 32], Error> {
    let signature_payload = TransactionSignaturePayload {
        network_id: hash(network_passphrase),
        tagged_transaction: TransactionSignaturePayloadTaggedTransaction::Tx(txn.clone()),
    };
    let hash = Sha256::digest(signature_payload.to_xdr(Limits::none())?).into();
    Ok(hash)
}

/// A trait for signing arbitrary byte arrays
#[async_trait::async_trait]
pub trait Blob {
    /// Sign an abritatry byte array
    async fn sign_blob(&self, blob: &[u8]) -> Result<Vec<u8>, Error>;
}

#[async_trait::async_trait]
pub trait TransactionHash {
    /// Sign a transaction hash with the given source account
    /// # Errors
    /// Returns an error if the source account is not found
    async fn sign_txn_hash(
        &self,
        source_account: &stellar_strkey::ed25519::PublicKey,
        txn: [u8; 32],
    ) -> Result<DecoratedSignature, Error>;
}
#[async_trait::async_trait]
impl<T> TransactionHash for T
where
    T: Blob + Send + Sync,
{
    async fn sign_txn_hash(
        &self,
        source_account: &stellar_strkey::ed25519::PublicKey,
        txn: [u8; 32],
    ) -> Result<DecoratedSignature, Error> {
        eprintln!(
            "{} about to sign hash: {}",
            source_account.to_string(),
            hex::encode(txn)
        );
        let tx_signature = self.sign_blob(&txn).await?;
        Ok(DecoratedSignature {
            // TODO: remove this unwrap. It's safe because we know the length of the array
            hint: SignatureHint(source_account.0[28..].try_into().unwrap()),
            signature: Signature(tx_signature.try_into()?),
        })
    }
}

/// A trait for signing Stellar transactions and Soroban authorization entries
#[async_trait::async_trait]
pub trait Transaction {
    /// Sign a Stellar transaction with the given source account
    /// This is a default implementation that signs the transaction hash and returns a decorated signature
    ///
    /// Todo: support signing the transaction directly.
    /// # Errors
    /// Returns an error if the source account is not found
    async fn sign_txn(
        &self,
        source_account: &stellar_strkey::ed25519::PublicKey,
        txn: &xdr::Transaction,
        network: &Network,
    ) -> Result<DecoratedSignature, Error>;
}

#[async_trait::async_trait]
impl<T> Transaction for T
where
    T: TransactionHash + Send + Sync,
{
    async fn sign_txn(
        &self,
        source_account: &stellar_strkey::ed25519::PublicKey,
        txn: &xdr::Transaction,
        Network {
            network_passphrase, ..
        }: &Network,
    ) -> Result<DecoratedSignature, Error> {
        let hash = transaction_hash(txn, network_passphrase)?;
        self.sign_txn_hash(source_account, hash).await
    }
}
pub async fn sign_txn_env(
    signer: &(impl Transaction + std::marker::Sync),
    source_account: &stellar_strkey::ed25519::PublicKey,
    txn_env: TransactionEnvelope,
    network: &Network,
) -> Result<TransactionEnvelope, Error> {
    match txn_env {
        TransactionEnvelope::Tx(TransactionV1Envelope { tx, signatures }) => {
            let decorated_signature = signer.sign_txn(source_account, &tx, network).await?;
            let mut sigs = signatures.to_vec();
            sigs.push(decorated_signature);
            Ok(TransactionEnvelope::Tx(TransactionV1Envelope {
                tx,
                signatures: sigs.try_into()?,
            }))
        }
        _ => Err(Error::UnsupportedTransactionEnvelopeType),
    }
}

pub(crate) fn hash(network_passphrase: &str) -> xdr::Hash {
    xdr::Hash(Sha256::digest(network_passphrase.as_bytes()).into())
}

pub struct LocalKey {
    key: ed25519_dalek::SigningKey,
    prompt: bool,
}

impl LocalKey {
    pub fn new(key: ed25519_dalek::SigningKey, prompt: bool) -> Self {
        Self { key, prompt }
    }
}

#[async_trait::async_trait]
impl Blob for LocalKey {
    async fn sign_blob(&self, data: &[u8]) -> Result<Vec<u8>, Error> {
        if self.prompt {
            eprintln!("Press 'y' or 'Y' for yes, any other key for no:");
            match read_key() {
                'y' | 'Y' => {
                    eprintln!("Signing now...");
                }
                _ => return Err(Error::UserCancelledSigning),
            };
        }
        let sig = self.key.sign(data);
        Ok(sig.to_bytes().to_vec())
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
