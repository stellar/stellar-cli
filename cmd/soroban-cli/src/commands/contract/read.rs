use std::{
    fmt::Debug,
    io::{self, stdout},
};

use clap::{ArgEnum, Parser};
use hex::FromHexError;
use soroban_env_host::{
    xdr::{
        self, ContractDataEntry, Error as XdrError, LedgerEntryData, LedgerKey,
        LedgerKeyContractData, ReadXdr, ScSpecTypeDef, ScVal, WriteXdr,
    },
    HostError,
};

use crate::{commands::config::ledger_file, strval, utils};

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

    #[clap(flatten)]
    ledger: ledger_file::Args,
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
    #[error(transparent)]
    Ledger(#[from] ledger_file::Error),
    #[error("parsing key {key}: {error}")]
    CannotParseKey { key: String, error: strval::Error },
    #[error("parsing XDR key {key}: {error}")]
    CannotParseXdrKey { key: String, error: XdrError },
    #[error("cannot parse contract ID {contract_id}: {error}")]
    CannotParseContractId {
        contract_id: String,
        error: FromHexError,
    },
    #[error("cannot print result {result:?}: {error}")]
    CannotPrintResult { result: ScVal, error: strval::Error },
    #[error("cannot print result {result:?}: {error}")]
    CannotPrintJsonResult {
        result: ScVal,
        error: serde_json::Error,
    },
    #[error("cannot print as csv: {error}")]
    CannotPrintAsCsv { error: csv::Error },
    #[error("cannot print: {error}")]
    CannotPrintFlush { error: io::Error },
    #[error("xdr processing error: {0}")]
    Xdr(#[from] XdrError),
    #[error(transparent)]
    // TODO: the Display impl of host errors is pretty user-unfriendly
    //       (it just calls Debug). I think we can do better than that
    Host(#[from] HostError),
}

impl Cmd {
    #[allow(clippy::too_many_lines)]
    pub fn run(&self) -> Result<(), Error> {
        let contract_id: [u8; 32] =
            utils::id_from_str(&self.contract_id).map_err(|e| Error::CannotParseContractId {
                contract_id: self.contract_id.clone(),
                error: e,
            })?;
        let key = if let Some(key) = &self.key {
            Some(
                strval::from_string_primitive(key, &ScSpecTypeDef::Symbol).map_err(|e| {
                    Error::CannotParseKey {
                        key: key.clone(),
                        error: e,
                    }
                })?,
            )
        } else if let Some(key) = &self.key_xdr {
            Some(
                ScVal::from_xdr_base64(key).map_err(|e| Error::CannotParseXdrKey {
                    key: key.clone(),
                    error: e,
                })?,
            )
        } else {
            None
        };

        let state = self.ledger.read()?;
        let ledger_entries = &state.ledger_entries;

        let contract_id = xdr::Hash(contract_id);
        let entries: Vec<ContractDataEntry> = if let Some(key) = key {
            ledger_entries
                .iter()
                .find(|(k, _)| {
                    k.as_ref()
                        == &LedgerKey::ContractData(LedgerKeyContractData {
                            contract_id: contract_id.clone(),
                            key: key.clone(),
                        })
                })
                .iter()
                .copied()
                .cloned()
                .filter_map(|val| {
                    if let LedgerEntryData::ContractData(d) = val.1.data {
                        Some(d)
                    } else {
                        None
                    }
                })
                .collect()
        } else {
            ledger_entries
                .iter()
                .filter_map(|(k, v)| {
                    if let LedgerKey::ContractData(kd) = *k.clone() {
                        if kd.contract_id == contract_id
                            && kd.key != ScVal::Static(xdr::ScStatic::LedgerKeyContractCode)
                        {
                            if let LedgerEntryData::ContractData(vd) = &v.data {
                                return Some(vd.clone());
                            }
                        }
                    }
                    None
                })
                .collect()
        };

        let mut out = csv::Writer::from_writer(stdout());
        for data in entries {
            let output = match self.output {
                Output::String => [
                    strval::to_string(&data.key).map_err(|e| Error::CannotPrintResult {
                        result: data.key.clone(),
                        error: e,
                    })?,
                    strval::to_string(&data.val).map_err(|e| Error::CannotPrintResult {
                        result: data.val.clone(),
                        error: e,
                    })?,
                ],
                Output::Json => [
                    serde_json::to_string_pretty(&data.key).map_err(|e| {
                        Error::CannotPrintJsonResult {
                            result: data.key.clone(),
                            error: e,
                        }
                    })?,
                    serde_json::to_string_pretty(&data.val).map_err(|e| {
                        Error::CannotPrintJsonResult {
                            result: data.val.clone(),
                            error: e,
                        }
                    })?,
                ],
                Output::Xdr => [data.key.to_xdr_base64()?, data.val.to_xdr_base64()?],
            };
            out.write_record(output)
                .map_err(|e| Error::CannotPrintAsCsv { error: e })?;
        }
        out.flush()
            .map_err(|e| Error::CannotPrintFlush { error: e })?;

        Ok(())
    }
}
