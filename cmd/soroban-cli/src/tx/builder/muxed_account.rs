use crate::xdr::{self, Uint256};

pub struct MuxedAccount(pub xdr::MuxedAccount);

impl From<&ed25519_dalek::VerifyingKey> for MuxedAccount {
    fn from(key: &ed25519_dalek::VerifyingKey) -> Self {
        MuxedAccount(xdr::MuxedAccount::Ed25519(Uint256(key.to_bytes())))
    }
}

impl From<&ed25519_dalek::SigningKey> for MuxedAccount {
    fn from(key: &ed25519_dalek::SigningKey) -> Self {
        key.verifying_key().into()
    }
}

impl From<ed25519_dalek::VerifyingKey> for MuxedAccount {
    fn from(key: ed25519_dalek::VerifyingKey) -> Self {
        (&key).into()
    }
}

impl From<ed25519_dalek::SigningKey> for MuxedAccount {
    fn from(key: ed25519_dalek::SigningKey) -> Self {
        key.verifying_key().into()
    }
}

impl From<stellar_strkey::ed25519::PublicKey> for MuxedAccount {
    fn from(key: stellar_strkey::ed25519::PublicKey) -> Self {
        MuxedAccount(xdr::MuxedAccount::Ed25519(Uint256(key.0)))
    }
}

impl From<&stellar_strkey::ed25519::PublicKey> for MuxedAccount {
    fn from(key: &stellar_strkey::ed25519::PublicKey) -> Self {
        MuxedAccount(xdr::MuxedAccount::Ed25519(Uint256(key.0)))
    }
}

impl From<MuxedAccount> for xdr::MuxedAccount {
    fn from(builder: MuxedAccount) -> Self {
        builder.0
    }
}
