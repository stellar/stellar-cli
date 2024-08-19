use crate::xdr;

use super::MuxedAccount;

pub mod create_account;
pub use create_account::CreateAccount;

pub mod payment;
pub use payment::Payment;

pub trait Operation: Sized {
    fn build_body(self) -> xdr::OperationBody;

    fn build_op<T: Into<MuxedAccount>>(self, source_account: Option<T>) -> xdr::Operation {
        operation(source_account, self.build_body())
    }
}

pub fn operation<T: Into<MuxedAccount>>(
    source_account: Option<T>,
    body: xdr::OperationBody,
) -> xdr::Operation {
    xdr::Operation {
        source_account: source_account.map(|s| s.into().into()),
        body,
    }
}
