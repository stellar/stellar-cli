use clap::{
    arg,
    builder::styling::{AnsiColor, Styles},
};
use std::path::PathBuf;

use super::config;

const USAGE_STYLES: Styles = Styles::styled()
    .header(AnsiColor::Red.on_default().bold())
    .usage(AnsiColor::Red.on_default().bold())
    .literal(AnsiColor::Blue.on_default().bold())
    .placeholder(AnsiColor::Green.on_default());

#[derive(Debug, clap::Args, Clone, Default)]
#[group(skip)]
#[allow(clippy::struct_excessive_bools)]
#[command(styles = USAGE_STYLES)]
pub struct Args {
    #[clap(flatten)]
    pub locator: config::locator::Args,

    /// Filter logs output. To turn on `stellar_cli::log::footprint=debug` or off `=off`. Can also use env var `RUST_LOG`.
    #[arg(long, short = 'f', global = true)]
    pub filter_logs: Vec<String>,

    /// Do not write logs to stderr including `INFO`
    #[arg(long, short = 'q', global = true)]
    pub quiet: bool,

    /// Log DEBUG events
    #[arg(long, short = 'v', global = true)]
    pub verbose: bool,

    /// Log DEBUG and TRACE events
    #[arg(long, visible_alias = "vv", global = true)]
    pub very_verbose: bool,

    /// List installed plugins. E.g. `stellar-hello`
    #[arg(long)]
    pub list: bool,

    /// Do not cache your simulations and transactions
    #[arg(long, env = "STELLAR_NO_CACHE", global = true)]
    pub no_cache: bool,
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
