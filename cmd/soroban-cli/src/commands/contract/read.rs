use std::{
    convert::Into,
    fmt::Debug,
    io::{self, stdout},
};

use clap::{command, Parser, ValueEnum};
use soroban_env_host::{
    xdr::{
        self, ContractDataDurability, ContractDataEntry, ContractDataEntryBody,
        ContractDataEntryData, ContractEntryBodyType, Error as XdrError, Hash, LedgerEntryData,
        LedgerKey, LedgerKeyContractData, ReadXdr, ScAddress, ScSpecTypeDef, ScVal, WriteXdr,
    },
    HostError,
};

use crate::{
    commands::config,
    commands::contract::Durability,
    rpc::{self, Client},
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
    /// Storage key (base64-encoded XDR ScVal)
    #[arg(long = "key-xdr", conflicts_with = "key")]
    key_xdr: Option<String>,
    /// Storage entry durability
    #[arg(long, value_enum)]
    durability: Option<Durability>,

    /// Type of output to generate
    #[arg(long, value_enum, default_value("string"))]
    output: Output,

    #[command(flatten)]
    config: config::Args,
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
    #[error(transparent)]
    Config(#[from] config::Error),
    #[error("either `--key` or `--key-xdr` are required when querying a network")]
    KeyIsRequired,
    #[error(transparent)]
    Rpc(#[from] rpc::Error),
    #[error("xdr processing error: {0}")]
    Xdr(#[from] XdrError),
    #[error(transparent)]
    // TODO: the Display impl of host errors is pretty user-unfriendly
    //       (it just calls Debug). I think we can do better than that
    Host(#[from] HostError),
}

impl Cmd {
    pub async fn run(&self) -> Result<(), Error> {
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

        let entries = if self.config.is_no_network() {
            self.run_in_sandbox(contract_id, &key)?
        } else {
            self.run_against_rpc_server(contract_id, key).await?
        };
        self.output_entries(&entries)
    }

    async fn run_against_rpc_server(
        &self,
        contract_id: [u8; 32],
        maybe_key: Option<ScVal>,
    ) -> Result<Vec<(LedgerKey, LedgerEntryData)>, Error> {
        let network = self.config.get_network()?;
        tracing::trace!(?network);
        let network = &self.config.get_network()?;
        let client = Client::new(&network.rpc_url)?;

        let key = maybe_key.ok_or(Error::KeyIsRequired)?;

        let keys: Vec<LedgerKey> = match self.durability {
            Some(Durability::Persistent) => {
                vec![Durability::Persistent]
            }
            Some(Durability::Temporary) => {
                vec![Durability::Temporary]
            }
            None => {
                vec![Durability::Persistent, Durability::Temporary]
            }
        }
        .iter()
        .map(|durability| {
            LedgerKey::ContractData(LedgerKeyContractData {
                contract: ScAddress::Contract(Hash(contract_id)),
                key: key.clone(),
                durability: (*durability).into(),
                body_type: ContractEntryBodyType::DataEntry,
            })
        })
        .collect::<Vec<_>>();

        tracing::trace!(?keys);

        client
            .get_ledger_entries(keys)
            .await?
            .entries
            .unwrap_or_default()
            .iter()
            .map(|result| {
                let key = LedgerKey::from_xdr_base64(result.key.as_bytes());
                let entry = LedgerEntryData::from_xdr_base64(result.xdr.as_bytes());
                match (key, entry) {
                    (Ok(k), Ok(e)) => Ok((k, e)),
                    (Err(e), _) | (_, Err(e)) => Err(e),
                }
            })
            .collect::<Result<Vec<(LedgerKey, LedgerEntryData)>, _>>()
            .map_err(Error::Xdr)
    }

    #[allow(clippy::too_many_lines)]
    fn run_in_sandbox(
        &self,
        contract_id: [u8; 32],
        key: &Option<ScVal>,
    ) -> Result<Vec<(LedgerKey, LedgerEntryData)>, Error> {
        let state = self.config.get_state()?;
        let ledger_entries = &state.ledger_entries;

        let contract = ScAddress::Contract(xdr::Hash(contract_id));
        let durability: Option<ContractDataDurability> = self.durability.map(Into::into);

        Ok(ledger_entries
            .iter()
            .map(|(k, v)| (k.as_ref().clone(), v.as_ref().clone()))
            .filter(|(k, _v)| {
                if let LedgerKey::ContractData(LedgerKeyContractData { contract: c, .. }) = k {
                    if c == &contract {
                        return true;
                    }
                }
                false
            })
            .filter(|(k, _v)| {
                if let LedgerKey::ContractData(LedgerKeyContractData { body_type, .. }) = k {
                    if body_type == &ContractEntryBodyType::DataEntry {
                        return true;
                    }
                }
                false
            })
            .filter(|(k, _v)| {
                if key.is_none() {
                    return true;
                }
                if let LedgerKey::ContractData(LedgerKeyContractData { key: k, .. }) = k {
                    if Some(k) == key.as_ref() {
                        return true;
                    }
                }
                false
            })
            .filter(|(k, _v)| {
                if durability.is_none() {
                    return true;
                }
                if let LedgerKey::ContractData(LedgerKeyContractData { durability: d, .. }) = k {
                    if Some(*d) == durability {
                        return true;
                    }
                }
                false
            })
            .map(|(k, v)| (k, v.data))
            .collect::<Vec<_>>())
    }

    fn output_entries(&self, raw_entries: &[(LedgerKey, LedgerEntryData)]) -> Result<(), Error> {
        let entries = raw_entries
            .iter()
            .filter_map(|(_k, data)| {
                if let LedgerEntryData::ContractData(ContractDataEntry {
                    key,
                    body: ContractDataEntryBody::DataEntry(ContractDataEntryData { val, .. }),
                    ..
                }) = &data
                {
                    Some((key.clone(), val.clone()))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

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
