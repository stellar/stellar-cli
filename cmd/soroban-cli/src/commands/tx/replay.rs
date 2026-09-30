use serde::Serialize;

use crate::{
    commands::{
        global,
        ledger::replay::{args, Error},
    },
    xdr::{Hash, LedgerCloseMeta, WriteXdr},
};

/// Replay a transaction with stellar-core and output its meta
///
/// Outputs the transaction's entry in its ledger's `LedgerCloseMeta`: the
/// transaction's result, the ledger entries it changed, and the events it
/// emitted, including diagnostic events that show the contract calls made and
/// the errors raised.
///
/// The transaction's ledger is replayed the same as with `ledger replay`.
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
        match &meta {
            LedgerCloseMeta::V0(m) => {
                self.print_tx(&m.tx_processing, |tx| &tx.result.transaction_hash)
            }
            LedgerCloseMeta::V1(m) => {
                self.print_tx(&m.tx_processing, |tx| &tx.result.transaction_hash)
            }
            LedgerCloseMeta::V2(m) => {
                self.print_tx(&m.tx_processing, |tx| &tx.result.transaction_hash)
            }
        }
    }

    /// Outputs the transaction with the hash from the ledger's transactions,
    /// which are a different type in each version of the meta.
    fn print_tx<'a, T: Serialize + WriteXdr + 'a>(
        &self,
        txs: impl IntoIterator<Item = &'a T>,
        hash: impl Fn(&T) -> &Hash,
    ) -> Result<(), Error> {
        let tx = txs
            .into_iter()
            .find(|tx| hash(tx) == &self.hash)
            .ok_or_else(|| Error::TxNotFound {
                hash: self.hash.clone(),
                ledger: self.ledger,
            })?;
        self.output.print(tx)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::xdr::{TransactionResultMetaV1, TransactionResultPair};

    fn cmd(hash: Hash) -> Cmd {
        Cmd {
            hash,
            ledger: 42,
            args: args::Args {
                network: crate::config::network::Args::default(),
                archive_url: None,
            },
            output: args::OutputFormat::default(),
        }
    }

    fn tx(hash: Hash) -> TransactionResultMetaV1 {
        TransactionResultMetaV1 {
            result: TransactionResultPair {
                transaction_hash: hash,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[test]
    fn test_print_tx() {
        let txs = [tx(Hash([1; 32])), tx(Hash([2; 32]))];
        cmd(Hash([2; 32]))
            .print_tx(&txs, |tx| &tx.result.transaction_hash)
            .unwrap();
    }

    #[test]
    fn test_print_tx_not_found() {
        let txs = [tx(Hash([1; 32]))];
        let err = cmd(Hash([3; 32]))
            .print_tx(&txs, |tx| &tx.result.transaction_hash)
            .unwrap_err();
        assert!(matches!(err, Error::TxNotFound { ledger: 42, .. }));
    }
}
