use ed25519_dalek::ed25519::signature::Signer;

use crate::{
    config::network::Network,
    xdr::{
        self, DecoratedSignature, Signature, SignatureHint, TransactionEnvelope,
        TransactionV1Envelope,
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

pub async fn sign_tx_env(
    signer: &(impl SignTx + std::marker::Sync),
    txn_env: TransactionEnvelope,
    network: &Network,
) -> Result<TransactionEnvelope, Error> {
    match txn_env {
        TransactionEnvelope::Tx(TransactionV1Envelope { tx, signatures }) => {
            let decorated_signature = signer.sign_tx(&tx, network).await?;
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

/// A trait for signing Stellar transactions and Soroban authorization entries
#[async_trait::async_trait]
pub trait SignTx {
    /// Sign a Stellar transaction with the given source account
    /// This is a default implementation that signs the transaction hash and returns a decorated signature
    ///
    /// Todo: support signing the transaction directly.
    /// # Errors
    /// Returns an error if the source account is not found
    async fn sign_tx(
        &self,
        txn: &xdr::Transaction,
        network: &Network,
    ) -> Result<DecoratedSignature, Error>;
}

pub struct LocalKey {
    key: ed25519_dalek::SigningKey,
    #[allow(dead_code)]
    prompt: bool,
}

impl LocalKey {
    pub fn new(key: ed25519_dalek::SigningKey, prompt: bool) -> Self {
        Self { key, prompt }
    }
}

impl LocalKey {
    pub fn sign_tx_hash(&self, tx_hash: [u8; 32]) -> Result<DecoratedSignature, Error> {
        let hint = SignatureHint(
            self.key.verifying_key().to_bytes()[28..]
                .try_into()
                .unwrap(),
        );
        let signature = Signature(self.key.sign(&tx_hash).to_bytes().to_vec().try_into()?);
        Ok(DecoratedSignature { hint, signature })
    }
}
