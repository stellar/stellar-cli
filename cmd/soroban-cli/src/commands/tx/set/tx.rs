use crate::{
    commands::global,
    xdr::{self, Transaction, TransactionEnvelope},
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Only transaction supported")]
    Unsupported,
}

#[derive(clap::Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    /// Set the transactions sequence number.
    #[arg(long, visible_alias = "seq_num")]
    pub sequence_number: Option<i64>,
    /// Set the transactions fee.
    #[arg(long)]
    pub fee: Option<u32>,
    /// Set the transactions max time bound
    #[arg(long)]
    pub max_time_bound: Option<u64>,
    /// Set the transactions min time bound
    #[arg(long)]
    pub min_time_bound: Option<u64>,
    /// Set the transactions memo text.
    #[arg(
        long,
        conflicts_with = "memo_id",
        conflicts_with = "memo_hash",
        conflicts_with = "memo_return"
    )]
    pub memo_text: Option<xdr::StringM<28>>,
    /// Set the transactions memo id.
    #[arg(
        long,
        conflicts_with = "memo_text",
        conflicts_with = "memo_hash",
        conflicts_with = "memo_return"
    )]
    pub memo_id: Option<u64>,
    /// Set the transactions memo hash.
    #[arg(
        long,
        conflicts_with = "memo_text",
        conflicts_with = "memo_id",
        conflicts_with = "memo_return"
    )]
    pub memo_hash: Option<xdr::Hash>,
    /// Set the transactions memo return.
    #[arg(
        long,
        conflicts_with = "memo_text",
        conflicts_with = "memo_id",
        conflicts_with = "memo_hash"
    )]
    pub memo_return: Option<xdr::Hash>,
}

impl Cmd {
    pub fn preconditions(&self) -> Option<crate::xdr::TimeBounds> {
        match (self.min_time_bound, self.max_time_bound) {
            (Some(min_time), Some(max_time)) => Some(crate::xdr::TimeBounds {
                min_time: min_time.into(),
                max_time: max_time.into(),
            }),
            (min, Some(max_time)) => Some(crate::xdr::TimeBounds {
                min_time: min.unwrap_or_default().into(),
                max_time: max_time.into(),
            }),
            (Some(min_time), max) => Some(crate::xdr::TimeBounds {
                min_time: min_time.into(),
                max_time: max.unwrap_or(u64::MAX).into(),
            }),
            _ => None,
        }
    }

    pub fn run(&self, _: &global::Args, tx: &mut TransactionEnvelope) -> Result<(), Error> {
        match tx {
            TransactionEnvelope::Tx(transaction_v1_envelope) => {
                let Transaction {
                    source_account,
                    fee,
                    seq_num,
                    cond,
                    memo,
                    operations,
                    ext,
                } = transaction_v1_envelope.tx.clone();

                if let Some(seq_num) = self.sequence_number {
                    transaction_v1_envelope.tx.seq_num = seq_num.into();
                }
                if let Some(fee) = self.fee {
                    transaction_v1_envelope.tx.fee = fee;
                }

                if let Some(memo) = self.memo_text.as_ref() {
                    transaction_v1_envelope.tx.memo = xdr::Memo::Text(memo.clone());
                }
                if let Some(memo) = self.memo_id {
                    transaction_v1_envelope.tx.memo = xdr::Memo::Id(memo);
                }
                if let Some(memo) = self.memo_hash.as_ref() {
                    transaction_v1_envelope.tx.memo = xdr::Memo::Hash(memo.clone());
                }
                if let Some(memo) = self.memo_return.as_ref() {
                    transaction_v1_envelope.tx.memo = xdr::Memo::Return(memo.clone());
                }
            }
            TransactionEnvelope::TxV0(_) | TransactionEnvelope::TxFeeBump(_) => {
                Err(Error::Unsupported)?
            }
        };
        Ok(())
    }
}
