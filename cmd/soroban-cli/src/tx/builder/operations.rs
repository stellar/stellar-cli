use crate::xdr;

use super::MuxedAccount;

mod account_merge;
mod allow_trust;
mod begin_sponsoring_future_reserves;
mod bump_sequence;
mod change_trust;
mod create_account;
mod end_sponsoring_future_reserves;
mod manage_data;
mod payment;
mod set_options;
mod set_trustline_flags;

pub use account_merge::*;
pub use allow_trust::*;
pub use begin_sponsoring_future_reserves::*;
pub use bump_sequence::*;
pub use change_trust::*;
pub use create_account::*;
pub use end_sponsoring_future_reserves::*;
pub use manage_data::*;
pub use payment::*;
pub use set_options::*;
pub use set_trustline_flags::*;

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
