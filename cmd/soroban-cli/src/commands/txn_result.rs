use crate::xdr::{Transaction, TransactionEnvelope, TransactionV1Envelope, VecM};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum TxnResult<R> {
    Txn(Transaction),
    Res(R),
}

impl<R> TxnResult<R> {
    pub fn into_result(self) -> Option<R> {
        match self {
            TxnResult::Res(res) => Some(res),
            TxnResult::Txn(_) => None,
        }
    }

    pub fn to_envelope(self) -> TxnEnvelopeResult<R> {
        match self {
            TxnResult::Txn(tx) => {
                TxnEnvelopeResult::TxnEnvelope(TransactionEnvelope::Tx(TransactionV1Envelope {
                    tx,
                    signatures: VecM::default(),
                }))
            }
            TxnResult::Res(res) => TxnEnvelopeResult::Res(res),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum TxnEnvelopeResult<R> {
    TxnEnvelope(TransactionEnvelope),
    Res(R),
}
