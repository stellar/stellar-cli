use std::{array::TryFromSliceError, fmt::Debug, num::ParseIntError, rc::Rc};

use clap::Parser;
// use rand::Rng;
// use sha2::{Digest, Sha256};
// use stellar_strkey::StrkeyPublicKeyEd25519;

use crate::{
    snapshot, HEADING_CONFIG, HEADING_RPC, HEADING_SANDBOX,
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("reading file {filepath}: {error}")]
    CannotReadLedgerFile {
        filepath: std::path::PathBuf,
        error: snapshot::Error,
    },
    #[error("committing file {filepath}: {error}")]
    CannotCommitLedgerFile {
        filepath: std::path::PathBuf,
        error: snapshot::Error,
    },
    #[error("cannot parse secret key")]
    CannotParseSecretKey,
    #[error("error parsing int: {0}")]
    ParseIntError(#[from] ParseIntError),
    #[error("internal conversion error: {0}")]
    TryFromSliceError(#[from] TryFromSliceError),
}

#[derive(Parser, Debug)]
pub struct Cmd {
    /// File to persist profile config
    #[clap(
        long,
        parse(from_os_str),
        default_value = "~/.config/soroban/config.json",
        env = "SOROBAN_CONFIG_FILE",
        help_heading = HEADING_CONFIG,
    )]
    config_file: std::path::PathBuf,

    /// Name of the profile, e.g. "sandbox"
    #[clap(long)]
    name: String,

    /// File to persist ledger state (if using the sandbox)
    #[clap(
        long,
        parse(from_os_str),
        default_value = ".soroban/ledger.json",
        conflicts_with = "rpc-url",
        env = "SOROBAN_LEDGER_FILE",
        help_heading = HEADING_SANDBOX,
    )]
    ledger_file: std::path::PathBuf,

    /// RPC server endpoint
    #[clap(
        long,
        requires = "secret-key",
        requires = "network-passphrase",
        env = "SOROBAN_RPC_URL",
        help_heading = HEADING_RPC,
    )]
    rpc_url: Option<String>,
    /// Secret key to sign the transaction sent to the rpc server
    #[clap(
        long = "secret-key",
        env = "SOROBAN_SECRET_KEY",
        help_heading = HEADING_RPC,
    )]
    secret_key: Option<String>,
    /// Network passphrase to sign the transaction sent to the rpc server
    #[clap(
        long = "network-passphrase",
        env = "SOROBAN_NETWORK_PASSPHRASE",
        help_heading = HEADING_RPC,
    )]
    network_passphrase: Option<String>,
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        todo!()
    }
}
