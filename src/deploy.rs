use std::{fmt::Debug, fs, io};

use clap::Parser;
use stellar_contract_env_host::xdr::{
    ContractDataEntry, Error as XdrError, LedgerEntry, LedgerEntryData, LedgerEntryExt, LedgerKey,
    LedgerKeyContractData, ScObject, ScStatic, ScVal,
};

use hex::{FromHex, FromHexError};

use crate::snapshot;

#[derive(Parser, Debug)]
pub struct Cmd {
    #[clap(long = "id")]
    /// Contract ID in Hexadecimal
    contract_id: String,
    /// File that contains a WASM contract
    #[clap(long, parse(from_os_str))]
    file: std::path::PathBuf,
    /// File to read and write ledger
    #[clap(long, parse(from_os_str), default_value("ledger.json"))]
    snapshot_file: std::path::PathBuf,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("io")]
    Io(#[from] io::Error),
    #[error("xdr")]
    Xdr(#[from] XdrError),
    #[error("snapshot")]
    Snapshot(#[from] snapshot::Error),
    #[error("hex")]
    FromHex(#[from] FromHexError),
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        let contract_id: [u8; 32] = FromHex::from_hex(&self.contract_id)?;
        let contract = fs::read(&self.file).unwrap();

        let key = LedgerKey::ContractData(LedgerKeyContractData {
            contract_id: contract_id.into(),
            key: ScVal::Static(ScStatic::LedgerKeyContractCodeWasm),
        });

        let data = LedgerEntryData::ContractData(ContractDataEntry {
            contract_id: contract_id.into(),
            key: ScVal::Static(ScStatic::LedgerKeyContractCodeWasm),
            val: ScVal::Object(Some(ScObject::Binary(contract.try_into()?))),
        });

        let entry = LedgerEntry {
            last_modified_ledger_seq: 0,
            data,
            ext: LedgerEntryExt::V0,
        };

        let mut ledger_entries = snapshot::read(&self.snapshot_file)?;
        ledger_entries.insert(key, entry);

        snapshot::commit(ledger_entries, None, &self.snapshot_file)?;
        Ok(())
    }
}
