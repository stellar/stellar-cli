use soroban_sdk::xdr::WriteXdr;

use crate::{
    commands::global,
    config::address::{self, UnresolvedMuxedAccount},
    xdr::{self, TransactionEnvelope},
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    XdrStdin(#[from] super::xdr::Error),
    #[error(transparent)]
    Xdr(#[from] xdr::Error),
    #[error(transparent)]
    Address(#[from] address::Error),
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
    /// Change the source account for the transaction
    #[arg(long, visible_alias = "source")]
    pub source_account: Option<UnresolvedMuxedAccount>,

    // Time bounds and Preconditions
    /// Set the transactions max time bound
    #[arg(long)]
    pub max_time_bound: Option<u64>,
    /// Set the transactions min time bound
    #[arg(long)]
    pub min_time_bound: Option<u64>,

    /// Set the minimum ledger that the transaction is valid
    #[arg(long)]
    pub min_ledger: Option<u32>,
    /// Set the max ledger that the transaction is valid. 0 or not present means to max
    #[arg(long)]
    pub max_ledger: Option<u32>,
    /// set mimimum sequence number
    #[arg(long)]
    pub min_seq_num: Option<i64>,
    // min sequence age in seconds
    #[arg(long)]
    pub min_seq_age: Option<u64>,
    /// min sequeence ledger gap
    #[arg(long)]
    pub min_seq_ledger_gap: Option<u32>,
    /// Extra signers
    #[arg(long, num_args = 0..=2)]
    pub extra_signers: Vec<xdr::SignerKey>,
    /// Set precondition to None
    #[arg(
        long,
        conflicts_with = "extra_signers",
        conflicts_with = "min_ledger",
        conflicts_with = "max_ledger",
        conflicts_with = "min_seq_num",
        conflicts_with = "min_seq_age",
        conflicts_with = "min_seq_ledger_gap",
        conflicts_with = "max_time_bound",
        conflicts_with = "min_time_bound"
    )]
    pub no_preconditions: bool,
}

impl Cmd {
    pub fn run(&self, global: &global::Args) -> Result<(), Error> {
        let mut tx = super::xdr::tx_envelope_from_stdin()?;
        self.update_tx_env(&mut tx, global)?;
        println!("{}", tx.to_xdr_base64(xdr::Limits::none())?);
        Ok(())
    }

    pub fn update_tx_env(
        &self,
        tx_env: &mut TransactionEnvelope,
        global: &global::Args,
    ) -> Result<(), Error> {
        match tx_env {
            TransactionEnvelope::Tx(transaction_v1_envelope) => {
                if let Some(source_account) = self.source_account.as_ref() {
                    transaction_v1_envelope.tx.source_account =
                        source_account.resolve_muxed_account_sync(&global.locator, None)?;
                };

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
                if let Some(preconditions) = self.preconditions()? {
                    transaction_v1_envelope.tx.cond = preconditions;
                }
            }
            TransactionEnvelope::TxV0(_) | TransactionEnvelope::TxFeeBump(_) => {
                return Err(Error::Unsupported);
            }
        };
        Ok(())
    }

    pub fn has_preconditionv2(&self) -> bool {
        self.min_ledger.is_some()
            || self.max_ledger.is_some()
            || self.min_seq_num.is_some()
            || self.min_seq_age.is_some()
            || self.min_seq_ledger_gap.is_some()
            || !self.extra_signers.is_empty()
    }

    pub fn preconditions(&self) -> Result<Option<xdr::Preconditions>, Error> {
        let time_bounds = self.timebounds();

        Ok(if self.no_preconditions {
            Some(xdr::Preconditions::None)
        } else if self.has_preconditionv2() {
            let ledger_bounds = if self.min_ledger.is_some() || self.max_ledger.is_some() {
                Some(xdr::LedgerBounds {
                    min_ledger: self.min_ledger.unwrap_or_default(),
                    max_ledger: self.max_ledger.unwrap_or_default(),
                })
            } else {
                None
            };
            Some(xdr::Preconditions::V2(xdr::PreconditionsV2 {
                ledger_bounds,
                time_bounds,
                min_seq_num: self.min_seq_num.map(xdr::SequenceNumber),
                min_seq_age: self.min_seq_age.unwrap_or_default().into(),
                min_seq_ledger_gap: self.min_seq_ledger_gap.unwrap_or_default(),
                extra_signers: self.extra_signers.clone().try_into()?,
            }))
        } else {
            None
        })
    }

    pub fn timebounds(&self) -> Option<crate::xdr::TimeBounds> {
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
}
