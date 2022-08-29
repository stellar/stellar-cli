use std::{
    fmt::Debug,
    fs::{create_dir_all, File, OpenOptions},
    io::{self, BufRead, BufReader, BufWriter, Write},
};

use clap::Parser;
use soroban_env_host::{
    events::{Events, HostEvent},
    xdr::{ContractEvent, Error as XdrError, Hash},
    HostError,
};

use hex::FromHexError;

use crate::strval::StrValError;
use crate::utils;

pub const FILE_NAME: &str = "events.log";

#[derive(Parser, Debug)]
pub struct Cmd {
    /// Contract ID to invoke
    #[clap(long = "id")]
    contract_id: Option<String>,
    /// Directory to persist ledger state
    #[clap(long, parse(from_os_str), default_value(".soroban"))]
    data_directory: std::path::PathBuf,
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
    #[error("serde")]
    Serde(#[from] serde_json::Error),
    #[error("hex")]
    FromHex(#[from] FromHexError),
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        let contract_id: Option<[u8; 32]> = match &self.contract_id {
            None => None,
            Some(id) => Some(utils::contract_id_from_str(id)?),
        };

        let file = match File::open(&self.data_directory.join(FILE_NAME)) {
            Ok(f) => f,
            Err(e) => {
                //File doesn't exist, so treat this as an empty database and the file will be created later
                if e.kind() == std::io::ErrorKind::NotFound {
                    return Ok(());
                }
                return Err(Error::Io(e));
            }
        };
        let buffered = BufReader::new(file);

        for line in buffered.lines() {
            let event: ContractEvent = serde_json::from_str(&line?)?;
            if let Some(filter_id) = contract_id {
                if event.contract_id != Some(Hash(filter_id)) {
                    continue;
                };
            };
            print(&event)?;
        }

        Ok(())
    }
}

pub fn commit(events: Events, data_directory: &std::path::Path, log: bool) -> Result<(), Error> {
    let output_file = data_directory.join(FILE_NAME);
    //Need to start off with the existing snapshot (new_state) since it's possible the storage_map did not touch every existing entry
    if let Some(dir) = output_file.parent() {
        if !dir.exists() {
            create_dir_all(dir)?;
        }
    }

    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(output_file)?;
    let mut out = BufWriter::new(file);
    for e in events.0.iter() {
        if let HostEvent::Contract(event) = e {
            // DebugEvents ignored
            if log {
                print(event)?;
            };
            let s = serde_json::to_string(&event)?;
            out.write_all(s.as_bytes())?;
            out.write_all(b"\n")?;
        };
    }
    out.flush()?;
    Ok(())
}

// TODO: Better print format here. support xdr? strval?
pub fn print(event: &ContractEvent) -> Result<(), Error> {
    eprintln!(
        "[Event] {}: {}",
        event
            .contract_id
            .as_ref()
            .map(hex::encode)
            .unwrap_or_else(|| "-".to_string()),
        serde_json::to_string(&event.body)?,
    );
    Ok(())
}
