use std::{fmt::Debug, fs, io};

use clap::Parser;
use soroban_env_host::xdr::Error as XdrError;

use hex::FromHexError;

use crate::snapshot;
use crate::utils;

#[derive(Parser, Debug)]
pub struct Cmd {
    #[clap(long = "id")]
    /// Contract ID to deploy to
    contract_id: String,
    /// WASM file to deploy
    #[clap(long, parse(from_os_str))]
    wasm: std::path::PathBuf,
    /// File to persist ledger state
    #[clap(long, parse(from_os_str), default_value("ledger.json"))]
    ledger_file: std::path::PathBuf,
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
        let contract_id: [u8; 32] = utils::contract_id_from_str(&self.contract_id)?;
        let contract = fs::read(&self.wasm).unwrap();

        let mut ledger_entries = snapshot::read(&self.ledger_file)?;
        utils::add_contract_to_ledger_entries(&mut ledger_entries, contract_id, contract)?;

        snapshot::commit(ledger_entries, None, &self.ledger_file)?;
        Ok(())
    }
}
