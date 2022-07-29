use std::{fs::File, io};

use soroban_env_host::{
    im_rc::OrdMap,
    storage::SnapshotSource,
    xdr::{Error as XdrError, LedgerEntry, LedgerKey, ScHostStorageErrorCode, ScStatus, VecM},
    HostError,
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("io")]
    Io(#[from] io::Error),
    #[error("xdr")]
    Xdr(#[from] XdrError),
    #[error("host")]
    Host(#[from] HostError),
    #[error("serde")]
    Serde(#[from] serde_json::Error),
}

pub struct Snap {
    pub ledger_entries: OrdMap<LedgerKey, LedgerEntry>,
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
pub fn read(input_file: &std::path::PathBuf) -> Result<OrdMap<LedgerKey, LedgerEntry>, Error> {
    let mut res = OrdMap::new();

    let mut file = match File::open(input_file) {
        Ok(f) => f,
        Err(e) => {
            //File doesn't exist, so treat this as an empty database and the file will be created later
            if e.kind() == std::io::ErrorKind::NotFound {
                return Ok(res);
            }
            return Err(Error::Io(e));
        }
    };

    let state: VecM<(LedgerKey, LedgerEntry)> = serde_json::from_reader(&mut file)?;
    res = state.iter().cloned().collect();

    Ok(res)
}

pub fn commit(
    mut new_state: OrdMap<LedgerKey, LedgerEntry>,
    storage_map: Option<&OrdMap<LedgerKey, Option<LedgerEntry>>>,
    output_file: &std::path::PathBuf,
) -> Result<(), Error> {
    //Need to start off with the existing snapshot (new_state) since it's possible the storage_map did not touch every existing entry
    let file = File::create(output_file)?;
    if let Some(s) = storage_map {
        for (lk, ole) in s {
            if let Some(le) = ole {
                new_state.insert(lk.clone(), le.clone());
            } else {
                new_state.remove(lk);
            }
        }
    }

    let vec_new_state: VecM<(LedgerKey, LedgerEntry)> =
        new_state.into_iter().collect::<Vec<_>>().try_into()?;
    serde_json::to_writer(&file, &vec_new_state)?;

    Ok(())
}
