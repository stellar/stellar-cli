use stellar_xdr::curr as xdr;

use crate::tx::{builder::AccountId, ONE_XLM};

pub struct CreateAccount {
    pub destination: xdr::AccountId,
    pub starting_balance: i64,
}

impl CreateAccount {
    /// Creates a new `CreateAccountOpBuilder` with the given destination and starting balance.
    /// The starting balance defaults to 1 XLM.
    pub fn new(destination: impl Into<AccountId>, starting_balance: Option<i64>) -> Self {
        Self {
            destination: destination.into().into(),
            starting_balance: starting_balance.unwrap_or(ONE_XLM),
        }
    }
}

impl super::Operation for CreateAccount {
    fn build_body(self) -> xdr::OperationBody {
        xdr::OperationBody::CreateAccount(xdr::CreateAccountOp {
            destination: self.destination,
            starting_balance: self.starting_balance,
        })
    }
}
