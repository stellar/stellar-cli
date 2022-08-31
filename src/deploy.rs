use std::{fmt::Debug, fs};

use clap::Parser;

use crate::error::CmdError;
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
    #[clap(long, parse(from_os_str), default_value(".soroban/ledger.json"))]
    ledger_file: std::path::PathBuf,
}

impl Cmd {
    pub fn run(&self) -> Result<(), CmdError> {
        let contract_id: [u8; 32] =
            utils::contract_id_from_str(&self.contract_id).map_err(|e| {
                CmdError::CannotParseContractID {
                    contract_id: self.contract_id.clone(),
                    error: e,
                }
            })?;
        let contract = fs::read(&self.wasm).map_err(|e| CmdError::CannotReadContractFile {
            filepath: self.wasm.clone(),
            error: e,
        })?;

        let mut ledger_entries =
            snapshot::read(&self.ledger_file).map_err(|e| CmdError::CannotReadLedgerFile {
                filepath: self.ledger_file.clone(),
                error: e,
            })?;
        utils::add_contract_to_ledger_entries(&mut ledger_entries, contract_id, contract)?;

        snapshot::commit(ledger_entries, [], &self.ledger_file).map_err(|e| {
            CmdError::CannotCommitLedgerFile {
                filepath: self.ledger_file.clone(),
                error: e,
            }
        })?;
        Ok(())
    }
}
