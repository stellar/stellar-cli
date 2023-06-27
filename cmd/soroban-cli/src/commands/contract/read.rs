use std::{
    fmt::Debug,
    io::{self, stdout},
};

use clap::{command, Parser, ValueEnum};
use soroban_env_host::{
    xdr::{
        self, ContractDataDurability, ContractDataEntry, ContractDataEntryBody,
        ContractDataEntryData, ContractEntryBodyType, Error as XdrError, LedgerEntryData,
        LedgerKey, LedgerKeyContractData, ReadXdr, ScAddress, ScSpecTypeDef, ScVal, WriteXdr,
    },
    HostError,
};

use crate::{
    commands::config::{ledger_file, locator},
    utils,
};

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    /// Contract ID to invoke
    #[arg(long = "id")]
    contract_id: String,
    /// Storage key (symbols only)
    #[arg(long = "key", conflicts_with = "key_xdr")]
    key: Option<String>,
    /// Storage key (base64-encoded XDR)
    #[arg(long = "key-xdr", conflicts_with = "key")]
    key_xdr: Option<String>,
    /// Type of output to generate
    #[arg(long, value_enum, default_value("string"))]
    output: Output,

    #[command(flatten)]
    ledger: ledger_file::Args,

    #[command(flatten)]
    locator: locator::Args,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, ValueEnum)]
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
    CannotParseKey {
        key: String,
        error: soroban_spec_tools::Error,
    },
    #[error("parsing XDR key {key}: {error}")]
    CannotParseXdrKey { key: String, error: XdrError },
    #[error("cannot parse contract ID {contract_id}: {error}")]
    CannotParseContractId {
        contract_id: String,
        error: stellar_strkey::DecodeError,
    },
    #[error("cannot print result {result:?}: {error}")]
    CannotPrintResult {
        result: ScVal,
        error: soroban_spec_tools::Error,
    },
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
    #[error(transparent)]
    Locator(#[from] locator::Error),
}

impl Cmd {
    #[allow(clippy::too_many_lines)]
    pub fn run(&self) -> Result<(), Error> {
        let contract_id: [u8; 32] =
            utils::contract_id_from_str(&self.contract_id).map_err(|e| {
                Error::CannotParseContractId {
                    contract_id: self.contract_id.clone(),
                    error: e,
                }
            })?;
        let key = if let Some(key) = &self.key {
            Some(
                soroban_spec_tools::from_string_primitive(key, &ScSpecTypeDef::Symbol).map_err(
                    |e| Error::CannotParseKey {
                        key: key.clone(),
                        error: e,
                    },
                )?,
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

        let state = self.ledger.read(&self.locator.config_dir()?)?;
        let ledger_entries = &state.ledger_entries;

        let contract = ScAddress::Contract(xdr::Hash(contract_id));
        let entries: Vec<(ScVal, ScVal)> = if let Some(key) = key {
            ledger_entries
                .iter()
                .find(|(k, _)| {
                    // TODO: Figure out how to allow looking up temporary entries for this command.
                    k.as_ref()
                        == &LedgerKey::ContractData(LedgerKeyContractData {
                            contract: contract.clone(),
                            key: key.clone(),
                            durability: ContractDataDurability::Persistent,
                            body_type: ContractEntryBodyType::DataEntry,
                        })
                })
                .iter()
                .copied()
                .cloned()
                .filter_map(|val| {
                    if let LedgerEntryData::ContractData(ContractDataEntry {
                        key,
                        body: ContractDataEntryBody::DataEntry(ContractDataEntryData { val, .. }),
                        ..
                    }) = val.1.data
                    {
                        Some((key, val))
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
                        if kd.contract == contract && kd.key != ScVal::LedgerKeyContractInstance {
                            if let LedgerEntryData::ContractData(ContractDataEntry {
                                body:
                                    ContractDataEntryBody::DataEntry(ContractDataEntryData {
                                        val, ..
                                    }),
                                ..
                            }) = &v.data
                            {
                                return Some((kd.key, val.clone()));
                            }
                        }
                    }
                    None
                })
                .collect()
        };

        let mut out = csv::Writer::from_writer(stdout());
        for (key, val) in entries {
            let output = match self.output {
                Output::String => [
                    soroban_spec_tools::to_string(&key).map_err(|e| Error::CannotPrintResult {
                        result: key.clone(),
                        error: e,
                    })?,
                    soroban_spec_tools::to_string(&val).map_err(|e| Error::CannotPrintResult {
                        result: val.clone(),
                        error: e,
                    })?,
                ],
                Output::Json => [
                    serde_json::to_string_pretty(&key).map_err(|e| {
                        Error::CannotPrintJsonResult {
                            result: key.clone(),
                            error: e,
                        }
                    })?,
                    serde_json::to_string_pretty(&val).map_err(|e| {
                        Error::CannotPrintJsonResult {
                            result: val.clone(),
                            error: e,
                        }
                    })?,
                ],
                Output::Xdr => [key.to_xdr_base64()?, val.to_xdr_base64()?],
            };
            out.write_record(output)
                .map_err(|e| Error::CannotPrintAsCsv { error: e })?;
        }
        out.flush()
            .map_err(|e| Error::CannotPrintFlush { error: e })?;

        Ok(())
    }
}
