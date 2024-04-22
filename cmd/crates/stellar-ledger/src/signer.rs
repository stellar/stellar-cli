use ed25519_dalek::Signer;
use ledger_transport::async_trait;
use sha2::{Digest, Sha256};
use soroban_env_host::xdr::{
    self, DecoratedSignature, Limits, Signature, SignatureHint, Transaction, TransactionEnvelope,
    TransactionSignaturePayload, TransactionSignaturePayloadTaggedTransaction,
    TransactionV1Envelope, WriteXdr,
};

use soroban_rpc::Error as RpcError;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    RpcError(#[from] RpcError),
}

/// A trait for signing Stellar transactions and Soroban authorization entries
#[async_trait]
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
struct DefaultSigner {
    network_passphrase: String,
    keypairs: Vec<ed25519_dalek::SigningKey>,
}

impl DefaultSigner {
    pub fn get_key(
        &self,
        key: &stellar_strkey::Strkey,
    ) -> Result<&ed25519_dalek::SigningKey, Error> {
        match key {
            stellar_strkey::Strkey::PublicKeyEd25519(stellar_strkey::ed25519::PublicKey(bytes)) => {
                self.keypairs
                    .iter()
                    .find(|k| k.verifying_key().to_bytes() == *bytes)
            }
            _ => None,
        }
        .ok_or_else(|| {
            Error::RpcError(RpcError::MissingSignerForAddress {
                address: key.to_string(),
            })
        })
    }
}

#[async_trait]
impl Stellar for DefaultSigner {
    type Init = Vec<ed25519_dalek::SigningKey>;
    fn new(network_passphrase: &str, options: Option<Vec<ed25519_dalek::SigningKey>>) -> Self {
        DefaultSigner {
            network_passphrase: network_passphrase.to_string(),
            keypairs: options.unwrap_or_default(),
        }
    }

    fn sign_txn_hash(
        &self,
        txn: [u8; 32],
        source_account: &stellar_strkey::Strkey,
    ) -> Result<DecoratedSignature, Error> {
        let source_account = self.get_key(source_account)?;
        let tx_signature = source_account.sign(&txn);
        Ok(DecoratedSignature {
            // TODO: remove this unwrap. It's safe because we know the length of the array
            hint: SignatureHint(
                source_account.verifying_key().to_bytes()[28..]
                    .try_into()
                    .unwrap(),
            ),
            signature: Signature(tx_signature.to_bytes().try_into().unwrap()), //FIXME: remove unwrap
        })
    }

    fn network_hash(&self) -> xdr::Hash {
        xdr::Hash(Sha256::digest(self.network_passphrase.as_bytes()).into())
    }
}
