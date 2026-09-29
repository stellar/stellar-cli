use super::{args, Error};
use crate::{
    commands::global,
    xdr::{Hash, LedgerCloseMeta},
};

/// Replay a transaction with stellar-core and output its meta
///
/// Outputs the transaction's entry in its ledger's `LedgerCloseMeta`: the
/// transaction's result, the ledger entries it changed, and the events it
/// emitted, including diagnostic events that show the contract calls made and
/// the errors raised.
///
/// The transaction's ledger is replayed the same as with `replay ledger`.
#[derive(Debug, clap::Parser)]
pub struct Cmd {
    /// Hash of the transaction to replay
    #[arg(long = "tx")]
    pub hash: Hash,

    /// Ledger sequence number of the ledger the transaction is in
    #[arg(long)]
    pub ledger: u32,

    #[command(flatten)]
    pub args: args::Args,

    /// Format of the output
    #[arg(long, value_enum, default_value_t)]
    pub output: args::OutputFormat,
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let meta = self.args.replay(self.ledger, global_args).await?;
        let hash = &self.hash;
        let not_found = || Error::TxNotFound {
            hash: hash.clone(),
            ledger: self.ledger,
        };
        match &meta {
            LedgerCloseMeta::V0(m) => self.output.print(
                m.tx_processing
                    .iter()
                    .find(|tx| &tx.result.transaction_hash == hash)
                    .ok_or_else(not_found)?,
            ),
            LedgerCloseMeta::V1(m) => self.output.print(
                m.tx_processing
                    .iter()
                    .find(|tx| &tx.result.transaction_hash == hash)
                    .ok_or_else(not_found)?,
            ),
            LedgerCloseMeta::V2(m) => self.output.print(
                m.tx_processing
                    .iter()
                    .find(|tx| &tx.result.transaction_hash == hash)
                    .ok_or_else(not_found)?,
            ),
        }
    }
}
