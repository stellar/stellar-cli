use std::{
    convert::Into,
    fmt::Debug,
    io::{self},
};

use clap::{command, Parser, ValueEnum};
use sha2::{Digest, Sha256};
use soroban_env_host::{
    xdr::{
        Error as XdrError, ExpirationEntry, Hash, ScVal, WriteXdr,
    },
    HostError,
};

use crate::{
    commands::config,
    key,
    rpc::{self, Client, FullLedgerEntries, FullLedgerEntry},
};

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    /// Type of output to generate
    #[arg(long, value_enum, default_value("string"))]
    pub output: Output,
    #[command(flatten)]
    pub key: key::Args,
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
    #[error(transparent)]
    Xdr(#[from] XdrError),
    #[error(transparent)]
    // TODO: the Display impl of host errors is pretty user-unfriendly
    //       (it just calls Debug). I think we can do better than that
    Host(#[from] HostError),
    #[error("no matching contract data entries were found for the specified contract id")]
    NoContractDataEntryFoundForContractID,
    #[error(transparent)]
    Key(#[from] key::Error),
}

impl Cmd {
    pub async fn run(&self) -> Result<(), Error> {
        let entries = if self.config.is_no_network() {
            self.run_in_sandbox()?
        } else {
            self.run_against_rpc_server().await?
        };
        self.output_entries(&entries)
    }

    async fn run_against_rpc_server(&self) -> Result<FullLedgerEntries, Error> {
        let network = self.config.get_network()?;
        tracing::trace!(?network);
        let network = &self.config.get_network()?;
        let client = Client::new(&network.rpc_url)?;
        let keys = self.key.parse_keys()?;
        tracing::trace!("{keys:#?}");
        Ok(client.get_full_ledger_entries(keys.as_slice()).await?)
    }

    #[allow(clippy::too_many_lines)]
    fn run_in_sandbox(&self) -> Result<FullLedgerEntries, Error> {
        let state = self.config.get_state()?;
        let ledger_entries = &state.ledger_entries;

        let keys = self.key.parse_keys()?;
        let entries = ledger_entries
            .iter()
            .map(|(k, v)| (k.as_ref().clone(), (v.0.as_ref().clone(), v.1)))
            .filter(|(k, _v)| keys.contains(k))
            .map(|(key, (v, expiration))| {
                Ok(FullLedgerEntry {
                    expiration: ExpirationEntry {
                        key_hash: Hash(Sha256::digest(key.to_xdr()?).into()),
                        expiration_ledger_seq: expiration.unwrap_or_default(),
                    },
                    key,
                    val: v.data,
                })
            })
            .collect::<Result<Vec<_>, Error>>()?;
        Ok(FullLedgerEntries {
            entries,
            latest_ledger: 0,
        })
    }

    fn output_entries(&self, raw_entries: &FullLedgerEntries) -> Result<(), Error> {
        println!("{raw_entries:#?}");
        // let entries = raw_entries
        //     .iter()
        //     .filter_map(|(_k, data)| {
        //         if let LedgerEntryData::ContractData(ContractDataEntry { key, val, .. }) = &data {
        //             Some((key.clone(), val.clone()))
        //         } else {
        //             None
        //         }
        //     })
        //     .collect::<Vec<_>>();

        // if entries.is_empty() {
        //     return Err(Error::NoContractDataEntryFoundForContractID);
        // }

        // let mut out = csv::Writer::from_writer(stdout());
        // for (key, val) in entries {
        //     let output = match self.output {
        //         Output::String => [
        //             soroban_spec_tools::to_string(&key).map_err(|e| Error::CannotPrintResult {
        //                 result: key.clone(),
        //                 error: e,
        //             })?,
        //             soroban_spec_tools::to_string(&val).map_err(|e| Error::CannotPrintResult {
        //                 result: val.clone(),
        //                 error: e,
        //             })?,
        //         ],
        //         Output::Json => [
        //             serde_json::to_string_pretty(&key).map_err(|e| {
        //                 Error::CannotPrintJsonResult {
        //                     result: key.clone(),
        //                     error: e,
        //                 }
        //             })?,
        //             serde_json::to_string_pretty(&val).map_err(|e| {
        //                 Error::CannotPrintJsonResult {
        //                     result: val.clone(),
        //                     error: e,
        //                 }
        //             })?,
        //         ],
        //         Output::Xdr => [key.to_xdr_base64()?, val.to_xdr_base64()?],
        //     };
        //     out.write_record(output)
        //         .map_err(|e| Error::CannotPrintAsCsv { error: e })?;
        // }
        // out.flush()
        //     .map_err(|e| Error::CannotPrintFlush { error: e })?;
        Ok(())
    }
}
