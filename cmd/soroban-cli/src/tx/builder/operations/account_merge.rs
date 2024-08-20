use stellar_xdr::curr as xdr;

use crate::tx::builder;

pub struct AccountMerge(xdr::MuxedAccount);

impl AccountMerge {
    pub fn new(account: impl Into<builder::MuxedAccount>) -> Self {
        Self(account.into().into())
    }
}

impl super::Operation for AccountMerge {
    fn build_body(self) -> xdr::OperationBody {
        xdr::OperationBody::AccountMerge(self.0)
    }
}
