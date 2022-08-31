use std::{fmt::Debug, rc::Rc};

use clap::{ArgEnum, Parser};
use soroban_env_host::{
    storage::Storage,
    xdr::{
        self, LedgerEntryData, LedgerKey, LedgerKeyContractData, ReadXdr, ScSpecTypeDef, ScVal,
        WriteXdr,
    },
};

use crate::error::CmdError;
use crate::snapshot;
use crate::strval::{self};
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

impl Cmd {
    pub fn run(&self) -> Result<(), CmdError> {
        let contract_id: [u8; 32] =
            utils::contract_id_from_str(&self.contract_id).map_err(|e| {
                CmdError::CannotParseContractID {
                    contract_id: self.contract_id.clone(),
                    error: e,
                }
            })?;
        let key = if let Some(key) = &self.key {
            strval::from_string(key, &ScSpecTypeDef::Symbol).map_err(|e| {
                CmdError::CannotParseKey {
                    key: key.clone(),
                    error: e,
                }
            })?
        } else if let Some(key) = &self.key_xdr {
            ScVal::from_xdr_base64(key.to_string()).map_err(|e| CmdError::CannotParseXDRKey {
                key: key.clone(),
                error: e,
            })?
        } else {
            return Err(CmdError::MissingKey);
        };

        // Initialize storage
        let ledger_entries =
            snapshot::read(&self.ledger_file).map_err(|e| CmdError::CannotReadLedgerFile {
                filepath: self.ledger_file.clone(),
                error: e,
            })?;

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
            Output::String => {
                let res_str =
                    strval::to_string(&value).map_err(|e| CmdError::CannotPrintResult {
                        result: value,
                        error: e,
                    })?;
                println!("{}", res_str);
            }
            Output::Json => {
                let res_str = serde_json::to_string_pretty(&value).map_err(|e| {
                    CmdError::CannotPrintJSONResult {
                        result: value,
                        error: e,
                    }
                })?;
                println!("{}", res_str);
            }
            Output::Xdr => println!("{}", value.to_xdr_base64()?),
        }

        Ok(())
    }
}
