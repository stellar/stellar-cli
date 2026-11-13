use std::{io, path::PathBuf, process::ExitStatus};

use crate::{
    commands::global,
    config::{data, network},
    xdr::{self, Hash},
};

pub mod args;

/// Replay a ledger with stellar-core and output its meta
///
/// Outputs the ledger's `LedgerCloseMeta`, the record of what closing the
/// ledger did: the transactions applied, their results, the ledger entries they
/// changed, and the events they emitted, including diagnostic events that show
/// the contract calls made and the errors raised.
///
/// The ledger is replayed by stellar-core, or by its docker image if
/// stellar-core isn't installed, from the network's history archive, starting
/// from the ledger state at the checkpoint before the ledger. For mainnet the
/// state is several GB to download and needs tens of GB of disk. The state is
/// kept in the `replay` directory of the CLI's OS cache directory, and a ledger
/// shortly after the last one replayed continues from it. Ledgers replayed are
/// also kept, and aren't replayed again. Nothing is removed automatically, so
/// delete the directory to free the space.
#[derive(Debug, clap::Parser)]
pub struct Cmd {
    /// Ledger sequence number to replay
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
        self.output.print(&meta)
    }
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Network(#[from] network::Error),
    #[error(transparent)]
    Data(#[from] data::Error),
    #[error("archive url not configured, use --archive-url")]
    ArchiveUrlNotConfigured,
    #[error("getting history archive state from {url}: {error}")]
    GettingHistory { url: String, error: reqwest::Error },
    #[error("ledger {ledger} is not in the history archive yet, the latest ledger in it is {latest}, archives are updated every 64 ledgers")]
    LedgerNotInArchive { ledger: u32, latest: u32 },
    #[error("stellar-core not found, install stellar-core or docker to replay ledgers: https://developers.stellar.org/docs/validators/admin-guide/installation")]
    StellarCoreNotFound,
    #[error("running stellar-core: {0}")]
    RunningStellarCore(io::Error),
    #[error("stellar-core failed with {status}, see its log at {}", log.display())]
    StellarCoreFailed { status: ExitStatus, log: PathBuf },
    #[error("ledger {0} is missing from the meta stellar-core wrote")]
    LedgerMissingFromMeta(u32),
    #[error("transaction {hash} not found in ledger {ledger}")]
    TxNotFound { hash: Hash, ledger: u32 },
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Xdr(#[from] xdr::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}
