use std::{fs::create_dir_all, fs::File, io, iter::IntoIterator};

use soroban_env_host::{
    im_rc::OrdMap,
    storage::SnapshotSource,
    xdr::{Error as XdrError, LedgerEntry, LedgerKey, ScHostStorageErrorCode, ScStatus, VecM},
    HostError, LedgerInfo,
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Xdr(#[from] XdrError),
    #[error(transparent)]
    Host(#[from] HostError),
    #[error(transparent)]
    Serde(#[from] serde_json::Error),
}

pub struct Snap {
    pub ledger_entries: OrdMap<LedgerKey, LedgerEntry>,
}

pub fn get_default_ledger_info() -> LedgerInfo {
    LedgerInfo {
        protocol_version: 19,
        sequence_number: 0,
        timestamp: 0,
        network_id: vec![0u8],
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SerializableState {
    pub ledger_entries: VecM<(LedgerKey, LedgerEntry)>,
    pub protocol_version: u32,
    pub sequence_number: u32,
    pub timestamp: u64,
    pub network_id: Vec<u8>,
}

impl SnapshotSource for Snap {
    fn get(&self, key: &LedgerKey) -> Result<LedgerEntry, HostError> {
        match self.ledger_entries.get(key) {
            Some(v) => Ok(v.clone()),
            None => Err(ScStatus::HostStorageError(ScHostStorageErrorCode::UnknownError).into()),
        }
    }
    fn has(&self, key: &LedgerKey) -> Result<bool, HostError> {
        Ok(self.ledger_entries.contains_key(key))
    }
}

// Ledger file format is the default serde JSON representation of VecM<(LedgerKey, LedgerEntry)>
pub fn read(
    input_file: &std::path::PathBuf,
) -> Result<(LedgerInfo, OrdMap<LedgerKey, LedgerEntry>), Error> {
    let mut entries = OrdMap::new();

    let mut file = match File::open(input_file) {
        Ok(f) => f,
        Err(e) => {
            //File doesn't exist, so treat this as an empty database and the file will be created later
            if e.kind() == io::ErrorKind::NotFound {
                return Ok((get_default_ledger_info(), entries));
            }
            return Err(Error::Io(e));
        }
    };

    let state: SerializableState = serde_json::from_reader(&mut file)?;
    entries = state.ledger_entries.iter().cloned().collect();
    let info = LedgerInfo {
        protocol_version: state.protocol_version,
        sequence_number: state.sequence_number,
        timestamp: state.timestamp,
        network_id: state.network_id,
    };
    Ok((info, entries))
}

pub fn commit<'a, I>(
    mut new_state: OrdMap<LedgerKey, LedgerEntry>,
    ledger_info: LedgerInfo,
    storage_map: I,
    output_file: &std::path::PathBuf,
) -> Result<(), Error>
where
    I: IntoIterator<Item = (&'a LedgerKey, &'a Option<LedgerEntry>)>,
{
    //Need to start off with the existing snapshot (new_state) since it's possible the storage_map did not touch every existing entry
    if let Some(dir) = output_file.parent() {
        if !dir.exists() {
            create_dir_all(dir)?;
        }
    }

    let file = File::create(output_file)?;
    for (lk, ole) in storage_map {
        if let Some(le) = ole {
            new_state.insert(lk.clone(), le.clone());
        } else {
            new_state.remove(lk);
        }
    }

    let vec_new_state: VecM<(LedgerKey, LedgerEntry)> =
        new_state.into_iter().collect::<Vec<_>>().try_into()?;

    let output = SerializableState {
        ledger_entries: vec_new_state,
        protocol_version: ledger_info.protocol_version,
        sequence_number: ledger_info.sequence_number,
        timestamp: ledger_info.timestamp,
        network_id: ledger_info.network_id,
    };
    serde_json::to_writer(&file, &output)?;

    Ok(())
}
