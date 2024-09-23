use soroban_env_host::xdr::{self, Memo, SequenceNumber, TransactionExt};

use super::{Error, MuxedAccount, Operation};

#[derive(Debug, Clone)]
pub struct Transaction {
    source_account: xdr::MuxedAccount,
    fee: u32,
    seq_num: SequenceNumber,
    memo: Option<Memo>,
    operations: Vec<xdr::Operation>,
}

impl Transaction {
    pub fn new(
        source_account: impl Into<MuxedAccount>,
        fee: u32,
        seq_num: impl Into<SequenceNumber>,
    ) -> Self {
        Transaction {
            source_account: source_account.into().into(),
            fee,
            seq_num: seq_num.into(),
            memo: None,
            operations: Vec::new(),
        }
    }

    #[must_use]
    pub fn set_memo(mut self, memo: Memo) -> Self {
        self.memo = Some(memo);
        self
    }

    #[must_use]
    pub fn add_operation_builder(
        mut self,
        operation: impl Operation,
        source_account: Option<impl Into<MuxedAccount>>,
    ) -> Self {
        self.operations.push(operation.build_op(source_account));
        self
    }

    #[must_use]
    pub fn add_operation(mut self, operation: xdr::Operation) -> Self {
        self.operations.push(operation);
        self
    }

    pub fn build(self) -> Result<xdr::Transaction, Error> {
        Ok(xdr::Transaction {
            source_account: self.source_account.clone(),
            fee: self.fee,
            seq_num: self.seq_num,
            cond: soroban_env_host::xdr::Preconditions::None,
            memo: self.memo.clone().unwrap_or(Memo::None),
            operations: self
                .operations
                .clone()
                .try_into()
                .map_err(|_| Error::TooManyOperations)?,
            ext: TransactionExt::V0,
        })
    }
}
