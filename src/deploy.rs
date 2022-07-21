use std::{fmt::Debug, fs, io};

use clap::Parser;
use stellar_contract_env_host::xdr::Error as XdrError;

use hex::{FromHex, FromHexError};

use crate::snapshot;
use crate::utils;

#[derive(Parser, Debug)]
pub struct Cmd {
    #[clap(long = "id")]
    /// Contract ID in Hexadecimal
    contract_id: String,
    /// File that contains a WASM contract
    #[clap(long, parse(from_os_str))]
    file: std::path::PathBuf,
    /// File to read and write ledger
    #[clap(long, parse(from_os_str), default_value("ledger.json"))]
    snapshot_file: std::path::PathBuf,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("io")]
    Io(#[from] io::Error),
    #[error("xdr")]
    Xdr(#[from] XdrError),
    #[error("snapshot")]
    Snapshot(#[from] snapshot::Error),
    #[error("hex")]
    FromHex(#[from] FromHexError),
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        let contract_id: [u8; 32] = FromHex::from_hex(&self.contract_id)?;
        let contract = fs::read(&self.file).unwrap();

        let mut ledger_entries = snapshot::read(&self.snapshot_file)?;
        utils::add_contract_to_ledger_entries(&mut ledger_entries, contract_id, contract)?;

        snapshot::commit(ledger_entries, None, &self.snapshot_file)?;
        Ok(())
    }
}
