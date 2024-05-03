use sha2::{Digest, Sha256};
use soroban_env_host::xdr::{
    self, DecoratedSignature, Limits, Transaction, TransactionEnvelope,
    TransactionSignaturePayload, TransactionSignaturePayloadTaggedTransaction,
    TransactionV1Envelope, WriteXdr,
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Xdr(#[from] xdr::Error),
    #[error("Error signing transaction {address}")]
    MissingSignerForAddress { address: String },
}

/// A trait for signing Stellar transactions and Soroban authorization entries
pub trait Stellar {
    /// The type of the options that can be passed when creating a new signer
    type Init;
    /// Create a new signer with the given network passphrase and options
    fn new(network_passphrase: &str, options: Option<Self::Init>) -> Self;

    /// Get the network hash
    fn network_hash(&self) -> xdr::Hash;

    /// Sign a transaction hash with the given source account
    /// # Errors
    /// Returns an error if the source account is not found
    fn sign_txn_hash(
        &self,
        txn: [u8; 32],
        source_account: &stellar_strkey::Strkey,
    ) -> Result<DecoratedSignature, Error>;

    /// Sign a Stellar transaction with the given source account
    /// This is a default implementation that signs the transaction hash and returns a decorated signature
    /// # Errors
    /// Returns an error if the source account is not found
    fn sign_txn(
        &self,
        txn: Transaction,
        source_account: &stellar_strkey::Strkey,
    ) -> Result<TransactionEnvelope, Error> {
        let signature_payload = TransactionSignaturePayload {
            network_id: self.network_hash(),
            tagged_transaction: TransactionSignaturePayloadTaggedTransaction::Tx(txn.clone()),
        };
        let hash = Sha256::digest(&signature_payload.to_xdr(Limits::none()).unwrap()).into(); //#fixme: remove unwrap
        let decorated_signature = self.sign_txn_hash(hash, source_account)?;
        Ok(TransactionEnvelope::Tx(TransactionV1Envelope {
            tx: txn,
            signatures: vec![decorated_signature].try_into().unwrap(), //fixme: remove unwrap
        }))
    }
}
