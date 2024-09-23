use ed25519_dalek::ed25519::signature::Signer;

use crate::{
    config::network::Network,
    print::Print,
    utils::transaction_hash,
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

pub struct StellarSigner {
    pub kind: SignerKind,
    pub printer: Print,
}

pub enum SignerKind {
    Local(LocalKey),
}

impl StellarSigner {
    pub fn sign_tx(
        &self,
        txn: &xdr::Transaction,
        network: &Network,
    ) -> Result<DecoratedSignature, Error> {
        let tx_hash = transaction_hash(txn, &network.network_passphrase)?;
        let hex_hash = hex::encode(tx_hash);
        self.printer
            .infoln(format!("Signing transaction with hash: {hex_hash}"));
        match &self.kind {
            SignerKind::Local(key) => key.sign_tx_hash(tx_hash),
        }
    }
}

pub async fn sign_tx_env(
    signer: &StellarSigner,
    txn_env: TransactionEnvelope,
    network: &Network,
) -> Result<TransactionEnvelope, Error> {
    match txn_env {
        TransactionEnvelope::Tx(TransactionV1Envelope { tx, signatures }) => {
            let decorated_signature = signer.sign_tx(&tx, network)?;
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
        let hint = SignatureHint(self.key.verifying_key().to_bytes()[28..].try_into()?);
        let signature = Signature(self.key.sign(&tx_hash).to_bytes().to_vec().try_into()?);
        Ok(DecoratedSignature { hint, signature })
    }
}
