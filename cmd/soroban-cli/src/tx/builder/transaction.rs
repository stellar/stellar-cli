use crate::xdr::{self, Memo, SequenceNumber, TransactionExt};

use super::Error;

pub trait TxExt {
    fn new_tx(
        source: xdr::MuxedAccount,
        fee: u32,
        seq_num: impl Into<SequenceNumber>,
        operation: xdr::Operation,
    ) -> xdr::Transaction;

    fn add_operation(self, operation: xdr::Operation) -> Result<xdr::Transaction, Error>;

    fn add_memo(self, memo: Memo) -> xdr::Transaction;

    fn add_cond(self, cond: xdr::Preconditions) -> xdr::Transaction;
}

impl TxExt for xdr::Transaction {
    fn new_tx(
        source_account: xdr::MuxedAccount,
        fee: u32,
        seq_num: impl Into<SequenceNumber>,
        operation: xdr::Operation,
    ) -> xdr::Transaction {
        xdr::Transaction {
            source_account,
            fee,
            seq_num: seq_num.into(),
            cond: soroban_env_host::xdr::Preconditions::None,
            memo: Memo::None,
           operations: [operation].try_into().unwrap(),
            ext: TransactionExt::V0,
        }
    }

    fn add_operation(mut self, operation: xdr::Operation) -> Result<Self, Error> {
        let mut ops = self.operations.to_vec();
        ops.push(operation);
        self.operations = ops.try_into().map_err(|_| Error::TooManyOperations)?;
        Ok(self)
    }

    fn add_memo(mut self, memo: Memo) -> Self {
        self.memo = memo;
        self
    }

    fn add_cond(self, cond: xdr::Preconditions) -> xdr::Transaction {
        xdr::Transaction { cond, ..self }
    }
}
