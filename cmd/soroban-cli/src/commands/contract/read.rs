use std::{
    fmt::Debug,
    io::{self, stdout},
};

use crate::xdr::{
    ContractDataEntry, Error as XdrError, LedgerEntryData, LedgerKey, LedgerKeyContractData,
    Limits, ScVal, WriteXdr,
};
use clap::{Parser, ValueEnum};

use crate::utils::XDR_DEPTH_LIMIT;
use crate::{
    config::{self, locator},
    key,
    rpc::{self, FullLedgerEntries, FullLedgerEntry},
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
    config: config::ArgsLocatorAndNetwork,
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
    #[error("no matching contract data entries were found for the specified contract id")]
    NoContractDataEntryFoundForContractID,
    #[error(transparent)]
    Key(#[from] key::Error),
    #[error("Only contract data and code keys are allowed")]
    OnlyDataAllowed,
    #[error(transparent)]
    Locator(#[from] locator::Error),
    #[error(transparent)]
    Network(#[from] config::network::Error),
}

impl Cmd {
    pub async fn run(&self) -> Result<(), Error> {
        let entries = self
            .execute(&config::Args {
                locator: self.config.locator.clone(),
                network: self.config.network.clone(),
                source_account: config::UnresolvedMuxedAccount::default(),
                sign_with: config::sign_with::Args::default(),
                fee: None,
                inclusion_fee: None,
            })
            .await?;
        self.output_entries(&entries)
    }

    pub async fn execute(&self, config: &config::Args) -> Result<FullLedgerEntries, Error> {
        let network = config.get_network()?;
        tracing::trace!(?network);
        let client = network.rpc_client()?;
        let keys = self.key.parse_keys(&config.locator, &network)?;
        Ok(client.get_full_ledger_entries(&keys).await?)
    }

    fn output_entries(&self, entries: &FullLedgerEntries) -> Result<(), Error> {
        if entries.entries.is_empty() {
            return Err(Error::NoContractDataEntryFoundForContractID);
        }
        tracing::trace!("{entries:#?}");
        let mut out = csv::Writer::from_writer(stdout());
        for entry in &entries.entries {
            out.write_record(Self::entry_record(self.output, entry)?)
                .map_err(|e| Error::CannotPrintAsCsv { error: e })?;
        }
        out.flush()
            .map_err(|e| Error::CannotPrintFlush { error: e })?;
        Ok(())
    }

    fn entry_record(output: Output, entry: &FullLedgerEntry) -> Result<[String; 4], Error> {
        let FullLedgerEntry {
            key,
            val,
            live_until_ledger_seq,
            last_modified_ledger,
        } = entry;
        let (
            LedgerKey::ContractData(LedgerKeyContractData { key, .. }),
            LedgerEntryData::ContractData(ContractDataEntry { val, .. }),
        ) = &(key, val)
        else {
            return Err(Error::OnlyDataAllowed);
        };
        let output = match output {
            // `to_string` returns raw, unescaped bytes for a top-level
            // `ScVal::Symbol`, so sanitize before writing to the terminal to
            // strip any control/escape sequences the entry may carry.
            Output::String => [
                soroban_spec_tools::sanitize(&soroban_spec_tools::to_string(key).map_err(|e| {
                    Error::CannotPrintResult {
                        result: key.clone(),
                        error: e,
                    }
                })?),
                soroban_spec_tools::sanitize(&soroban_spec_tools::to_string(val).map_err(|e| {
                    Error::CannotPrintResult {
                        result: val.clone(),
                        error: e,
                    }
                })?),
                last_modified_ledger.to_string(),
                live_until_ledger_seq.unwrap_or_default().to_string(),
            ],
            Output::Json => [
                serde_json::to_string_pretty(&key).map_err(|error| {
                    Error::CannotPrintJsonResult {
                        result: key.clone(),
                        error,
                    }
                })?,
                serde_json::to_string_pretty(&val).map_err(|error| {
                    Error::CannotPrintJsonResult {
                        result: val.clone(),
                        error,
                    }
                })?,
                serde_json::to_string_pretty(&last_modified_ledger).map_err(|error| {
                    Error::CannotPrintJsonResult {
                        result: val.clone(),
                        error,
                    }
                })?,
                serde_json::to_string_pretty(&live_until_ledger_seq.unwrap_or_default()).map_err(
                    |error| Error::CannotPrintJsonResult {
                        result: val.clone(),
                        error,
                    },
                )?,
            ],
            Output::Xdr => [
                key.to_xdr_base64(Limits::depth(XDR_DEPTH_LIMIT))?,
                val.to_xdr_base64(Limits::depth(XDR_DEPTH_LIMIT))?,
                last_modified_ledger.to_xdr_base64(Limits::depth(XDR_DEPTH_LIMIT))?,
                live_until_ledger_seq
                    .unwrap_or_default()
                    .to_xdr_base64(Limits::depth(XDR_DEPTH_LIMIT))?,
            ],
        };
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xdr::{
        ContractDataDurability, ContractId, ExtensionPoint, Hash, ScAddress, ScSymbol, StringM,
    };

    fn symbol(bytes: &[u8]) -> ScVal {
        let s: StringM<32> = bytes.to_vec().try_into().unwrap();
        ScVal::Symbol(ScSymbol(s))
    }

    fn contract_data_entry(key: ScVal, val: ScVal) -> FullLedgerEntry {
        let contract = ScAddress::Contract(ContractId(Hash([0u8; 32])));
        FullLedgerEntry {
            key: LedgerKey::ContractData(LedgerKeyContractData {
                contract: contract.clone(),
                key,
                durability: ContractDataDurability::Persistent,
            }),
            val: LedgerEntryData::ContractData(ContractDataEntry {
                ext: ExtensionPoint::V0,
                contract,
                key: ScVal::Void,
                durability: ContractDataDurability::Persistent,
                val,
            }),
            last_modified_ledger: 2026,
            live_until_ledger_seq: Some(3_000_000),
        }
    }

    // A top-level `ScVal::Symbol` is rendered by `to_string` as raw, unescaped
    // bytes. The default (`string`) output must not let those control/escape
    // sequences reach the terminal.
    #[test]
    fn string_output_strips_control_bytes_from_symbol() {
        let attack = b"\x1b[2J\x1b[Hpwned";
        let entry = contract_data_entry(symbol(attack), symbol(attack));

        let record = Cmd::entry_record(Output::String, &entry).unwrap();

        assert!(
            !record[0].as_bytes().contains(&0x1b),
            "key field leaked an ESC byte: {:?}",
            record[0]
        );
        assert!(
            !record[1].as_bytes().contains(&0x1b),
            "val field leaked an ESC byte: {:?}",
            record[1]
        );
        // The (escaped) payload text is still present, just inert.
        assert!(record[0].contains("pwned"));
    }
}
