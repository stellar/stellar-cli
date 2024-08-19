use soroban_sdk::xdr::{OperationBody, PaymentOp};

use crate::tx::builder::{Asset, MuxedAccount};
use crate::xdr;

use super::Operation;

pub struct Payment(pub PaymentOp);

impl Payment {
    pub fn new(
        destination: impl Into<MuxedAccount>,
        asset: Asset,
        amount: i64,
    ) -> Result<Self, super::super::asset::Error> {
        Ok(Self(PaymentOp {
            destination: destination.into().into(),
            asset: asset.into(),
            amount,
        }))
    }
}
impl Operation for Payment {
    fn build_body(self) -> xdr::OperationBody {
        OperationBody::Payment(self.0)
    }
}
