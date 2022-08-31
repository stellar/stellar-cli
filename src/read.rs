use std::{fmt::Debug, io, rc::Rc};

use clap::{ArgEnum, Parser};
use soroban_env_host::{
    storage::Storage,
    xdr::{
        self, Error as XdrError, LedgerEntryData, LedgerKey, LedgerKeyContractData, ReadXdr,
        ScSpecTypeDef, ScVal, WriteXdr,
    },
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
    /// Storage key (symbols only)
    #[clap(long = "key", conflicts_with = "key-xdr")]
    key: Option<String>,
    /// Storage key (base64-encoded XDR)
    #[clap(long = "key-xdr", conflicts_with = "key")]
    key_xdr: Option<String>,
    /// Type of output to generate
    #[clap(long, arg_enum, default_value("string"))]
    output: Output,
    /// File to persist ledger state
    #[clap(long, parse(from_os_str), default_value(".soroban/ledger.json"))]
    ledger_file: std::path::PathBuf,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, ArgEnum)]
pub enum Output {
    /// String
    String,
    /// Json
    Json,
    /// XDR
    Xdr,
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
        let key = if let Some(key) = &self.key {
            strval::from_string(key, &ScSpecTypeDef::Symbol)?
        } else if let Some(key) = &self.key_xdr {
            ScVal::from_xdr_base64(key.to_string())?
        } else {
            return Err(Error::StrVal(StrValError::InvalidValue));
        };

        // Initialize storage
        let ledger_entries = snapshot::read(&self.ledger_file)?;

        let snap = Rc::new(snapshot::Snap { ledger_entries });
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

        match self.output {
            Output::String => println!("{}", strval::to_string(&value)?),
            Output::Json => println!("{}", serde_json::to_string_pretty(&value)?),
            Output::Xdr => println!("{}", value.to_xdr_base64()?),
        }

        Ok(())
    }
}
