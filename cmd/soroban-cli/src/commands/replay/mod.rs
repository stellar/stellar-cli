use std::{io, path::PathBuf, process::ExitStatus};

use crate::{
    commands::global,
    config::{data, network},
    xdr::{self, Hash},
};

pub mod args;
pub mod ledger;
pub mod tx;

#[derive(Debug, clap::Subcommand)]
pub enum Cmd {
    Ledger(ledger::Cmd),
    Tx(tx::Cmd),
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

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        match self {
            Cmd::Ledger(cmd) => cmd.run(global_args).await,
            Cmd::Tx(cmd) => cmd.run(global_args).await,
        }
    }
}
