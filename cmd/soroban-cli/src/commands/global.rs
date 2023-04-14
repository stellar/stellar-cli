use clap::arg;
use std::path::PathBuf;

use super::config;

#[derive(Debug, clap::Args, Clone, Default)]
#[group(skip)]
#[allow(clippy::struct_excessive_bools)]
pub struct Args {
    #[clap(flatten)]
    pub locator: config::locator::Args,

    /// Filter logs output
    #[arg(long, short = 'f')]
    pub filter_logs: Option<String>,

    /// Do not write logs to stderr
    #[arg(long, short = 'q')]
    pub quiet: bool,

    /// Log DEBUG events
    #[arg(long, short = 'v')]
    pub verbose: bool,

    /// Log DEBUG and TRACE events
    #[arg(long, alias = "vv")]
    pub very_verbose: bool,

    /// Write the output of the logs to a file
    #[arg(long, alias = "log-to")]
    pub log_file: Option<PathBuf>,

    /// List installed plugins. E.g. `soroban-hello`
    #[arg(long)]
    pub list: bool,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("reading file {filepath}: {error}")]
    CannotReadLedgerFile {
        filepath: PathBuf,
        error: soroban_ledger_snapshot::Error,
    },

    #[error("committing file {filepath}: {error}")]
    CannotCommitLedgerFile {
        filepath: PathBuf,
        error: soroban_ledger_snapshot::Error,
    },
}

impl Args {
    pub fn log_level(&self) -> Option<tracing::Level> {
        if self.quiet {
            None
        } else if self.very_verbose {
            Some(tracing::Level::TRACE)
        } else if self.verbose {
            Some(tracing::Level::DEBUG)
        } else {
            Some(tracing::Level::INFO)
        }
    }
}
