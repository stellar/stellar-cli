use soroban_sdk::xdr::{OperationBody, PaymentOp};

use crate::tx::builder::MuxedAccount;
use crate::xdr;

use super::Operation;

pub struct Payment(pub PaymentOp);

impl Payment {
    pub fn new(destination: impl Into<MuxedAccount>, asset: xdr::Asset, amount: i64) -> Self {
        Self(PaymentOp {
            destination: destination.into().into(),
            asset,
            amount,
        })
    }
}
impl Operation for Payment {
    fn build_body(self) -> xdr::OperationBody {
        OperationBody::Payment(self.0)
    }
}
