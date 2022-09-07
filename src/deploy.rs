use std::{fmt::Debug, fs, io};

use clap::Parser;
use hex::FromHexError;
use soroban_env_host::{xdr::Error as XdrError, HostError};

use crate::snapshot::{self, get_default_ledger_info};
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

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Host(#[from] HostError),
    #[error("xdr processing error: {0}")]
    Xdr(#[from] XdrError),
    #[error("reading file {filepath}: {error}")]
    CannotReadLedgerFile {
        filepath: std::path::PathBuf,
        error: snapshot::Error,
    },
    #[error("reading file {filepath}: {error}")]
    CannotReadContractFile {
        filepath: std::path::PathBuf,
        error: io::Error,
    },
    #[error("committing file {filepath}: {error}")]
    CannotCommitLedgerFile {
        filepath: std::path::PathBuf,
        error: snapshot::Error,
    },
    #[error("cannot parse contract ID {contract_id}: {error}")]
    CannotParseContractId {
        contract_id: String,
        error: FromHexError,
    },
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        let contract_id: [u8; 32] =
            utils::contract_id_from_str(&self.contract_id).map_err(|e| {
                Error::CannotParseContractId {
                    contract_id: self.contract_id.clone(),
                    error: e,
                }
            })?;
        let contract = fs::read(&self.wasm).map_err(|e| Error::CannotReadContractFile {
            filepath: self.wasm.clone(),
            error: e,
        })?;

        let mut state =
            snapshot::read(&self.ledger_file).map_err(|e| Error::CannotReadLedgerFile {
                filepath: self.ledger_file.clone(),
                error: e,
            })?;
        utils::add_contract_to_ledger_entries(&mut state.1, contract_id, contract)?;

        snapshot::commit(state.1, get_default_ledger_info(), [], &self.ledger_file).map_err(
            |e| Error::CannotCommitLedgerFile {
                filepath: self.ledger_file.clone(),
                error: e,
            },
        )?;
        Ok(())
    }
}
