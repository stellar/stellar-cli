use std::str::FromStr;

use crate::xdr;

/// Address can be either a public key or eventually an alias of a address.
#[derive(Clone, Debug)]
pub enum SignerKey {
    Ed25519(stellar_strkey::ed25519::PublicKey),
    PreAuthTx(stellar_strkey::PreAuthTx),
    HashX(stellar_strkey::HashX),
    Ed25519SignedPayload(stellar_strkey::ed25519::SignedPayload),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Strkey(#[from] stellar_strkey::DecodeError),
    #[error("Only Ed25519, PreAuthTx, HashX, and SignedPayloads are supported")]
    InvalidSignerKey,
}

impl FromStr for SignerKey {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match stellar_strkey::Strkey::from_string(value)? {
            stellar_strkey::Strkey::PublicKeyEd25519(public_key) => {
                Ok(SignerKey::Ed25519(public_key))
            }
            stellar_strkey::Strkey::PreAuthTx(muxed_account) => {
                Ok(SignerKey::PreAuthTx(muxed_account))
            }
            stellar_strkey::Strkey::HashX(hash_x) => Ok(SignerKey::HashX(hash_x)),
            stellar_strkey::Strkey::SignedPayloadEd25519(signed_payload) => {
                Ok(SignerKey::Ed25519SignedPayload(signed_payload))
            }
            _ => Err(Error::InvalidSignerKey),
        }
    }
}

impl From<SignerKey> for xdr::SignerKey {
    fn from(key: SignerKey) -> Self {
        match key {
            SignerKey::Ed25519(key) => xdr::SignerKey::Ed25519(key.0.into()),
            SignerKey::PreAuthTx(pre_auth_tx) => xdr::SignerKey::PreAuthTx(pre_auth_tx.0.into()),

            SignerKey::HashX(hash_x) => xdr::SignerKey::HashX(hash_x.0.into()),
            SignerKey::Ed25519SignedPayload(signed_payload) => {
                xdr::SignerKey::Ed25519SignedPayload(xdr::SignerKeyEd25519SignedPayload {
                    ed25519: signed_payload.ed25519.into(),
                    payload: signed_payload.payload.try_into().unwrap(),
                })
            }
        }
    }
}
