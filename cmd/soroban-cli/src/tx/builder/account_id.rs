use std::str::FromStr;

use crate::xdr;

#[derive(Clone, Debug)]
pub struct AccountId(pub xdr::AccountId);

impl From<AccountId> for xdr::AccountId {
    fn from(builder: AccountId) -> Self {
        builder.0
    }
}

impl From<&AccountId> for AccountId {
    fn from(account_id: &AccountId) -> Self {
        Self(account_id.0.clone())
    }
}

impl From<stellar_strkey::ed25519::PublicKey> for AccountId {
    fn from(key: stellar_strkey::ed25519::PublicKey) -> Self {
        AccountId(xdr::AccountId(xdr::PublicKey::PublicKeyTypeEd25519(
            key.0.into(),
        )))
    }
}
impl From<&stellar_strkey::ed25519::PublicKey> for AccountId {
    fn from(key: &stellar_strkey::ed25519::PublicKey) -> Self {
        AccountId(xdr::AccountId(xdr::PublicKey::PublicKeyTypeEd25519(
            key.0.into(),
        )))
    }
}

impl FromStr for AccountId {
    type Err = stellar_strkey::DecodeError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(stellar_strkey::ed25519::PublicKey::from_str(s)?.into())
    }
}
