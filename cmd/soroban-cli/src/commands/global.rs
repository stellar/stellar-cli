use clap::{
    arg,
    builder::styling::{AnsiColor, Effects, Styles},
};
use std::path::PathBuf;

use super::config;

const USAGE_STYLES: Styles = Styles::styled()
    .header(AnsiColor::Green.on_default().effects(Effects::BOLD))
    .usage(AnsiColor::Green.on_default().effects(Effects::BOLD))
    .literal(AnsiColor::Cyan.on_default().effects(Effects::BOLD))
    .placeholder(AnsiColor::Cyan.on_default().effects(Effects::BOLD))
    .error(AnsiColor::Red.on_default().effects(Effects::BOLD))
    .valid(AnsiColor::Cyan.on_default().effects(Effects::BOLD))
    .invalid(AnsiColor::Yellow.on_default().effects(Effects::BOLD));

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

    /// RPC URL for the Stellar network
    #[arg(long, env = "STELLAR_RPC_URL")]
    pub rpc_url: Option<String>,
 
    /// Network passphrase for the Stellar network
    #[arg(long, env = "STELLAR_NETWORK_PASSPHRASE")]
    pub network_passphrase: Option<String>,
 
    /// Network name (e.g., 'testnet', 'mainnet')
    #[arg(long, env = "STELLAR_NETWORK")]
    pub network: Option<String>,

    /// Path to the WebAssembly file
    #[arg(long, env = "STELLAR_WASM", global = true)]
    pub wasm: Option<PathBuf>,
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

    #[error("network arg or rpc url and network passphrase are required if using the network")]
    Network,
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
