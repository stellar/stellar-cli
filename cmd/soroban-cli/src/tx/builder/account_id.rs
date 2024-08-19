use crate::xdr;

pub struct AccountId(pub xdr::AccountId);

impl From<AccountId> for xdr::AccountId {
    fn from(builder: AccountId) -> Self {
        builder.0
    }
}

impl From<stellar_strkey::ed25519::PublicKey> for AccountId {
    fn from(key: stellar_strkey::ed25519::PublicKey) -> Self {
        AccountId(xdr::AccountId(xdr::PublicKey::PublicKeyTypeEd25519(
            xdr::Uint256(key.0),
        )))
    }
}
