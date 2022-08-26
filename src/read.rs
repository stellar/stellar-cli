use std::{fmt::Debug, io, rc::Rc};

use clap::Parser;
use soroban_env_host::{
    storage::Storage,
    xdr::{self, Error as XdrError, ReadXdr, WriteXdr, LedgerEntryData, LedgerKey, LedgerKeyContractData, ScVal},
    HostError,
};

use hex::FromHexError;

use crate::snapshot;
use crate::strval::{self, StrValError};
use crate::utils;

#[derive(Parser, Debug)]
pub struct Cmd {
    /// Contract ID to invoke
    #[clap(long = "id")]
    contract_id: String,
    /// Storage key to read from, base64-encoded xdr
    #[clap(long = "key")]
    key: String,
    /// Output the result as json, instead of base64-encoded xdr
    #[clap(long = "json")]
    json: bool,
    /// File to persist ledger state
    #[clap(long, parse(from_os_str), default_value(".soroban/ledger.json"))]
    ledger_file: std::path::PathBuf,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("io")]
    Io(#[from] io::Error),
    #[error("strval")]
    StrVal(#[from] StrValError),
    #[error("xdr")]
    Xdr(#[from] XdrError),
    #[error("host")]
    Host(#[from] HostError),
    #[error("snapshot")]
    Snapshot(#[from] snapshot::Error),
    #[error("serde")]
    Serde(#[from] serde_json::Error),
    #[error("hex")]
    FromHex(#[from] FromHexError),
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        let contract_id: [u8; 32] = utils::contract_id_from_str(&self.contract_id)?;
        let key = ScVal::from_xdr_base64(self.key.clone())?;

        // Initialize storage
        let ledger_entries = snapshot::read(&self.ledger_file)?;

        let snap = Rc::new(snapshot::Snap {
            ledger_entries: ledger_entries.clone(),
        });
        let mut storage = Storage::with_recording_footprint(snap);
        let ledger_entry = storage.get(&LedgerKey::ContractData(LedgerKeyContractData {
            contract_id: xdr::Hash(contract_id),
            key,
        }))?;

        let value = if let LedgerEntryData::ContractData(entry) = ledger_entry.data {
            entry.val
        } else {
            unreachable!();
        };

        if self.json {
            println!("{}", strval::to_string(&value)?);
        } else {
            println!("{}", value.to_xdr_base64()?);
        }

        Ok(())
    }
}
