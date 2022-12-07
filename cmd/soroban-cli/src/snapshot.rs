use std::{fs::create_dir_all, fs::File, io, iter::IntoIterator};

use soroban_env_host::{
    events,
    im_rc::OrdMap,
    storage::SnapshotSource,
    xdr::{
        self, Error as XdrError, LedgerEntry, LedgerKey, ScHostStorageErrorCode, ScStatus, VecM,
        WriteXdr,
    },
    HostError, LedgerInfo,
};

use crate::network::SANDBOX_NETWORK_PASSPHRASE;
use crate::rpc;

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
        network_passphrase: SANDBOX_NETWORK_PASSPHRASE.as_bytes().to_vec(),
        base_reserve: 0,
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct SerializableState {
    ledger_entries: VecM<(LedgerKey, LedgerEntry)>,
    protocol_version: u32,
    sequence_number: u32,
    timestamp: u64,
    network_passphrase: Vec<u8>,
    base_reserve: u32,
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
            // File doesn't exist, so treat this as an empty database and the
            // file will be created later
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
        network_passphrase: state.network_passphrase,
        base_reserve: state.base_reserve,
    };
    Ok((info, entries))
}

pub fn commit<'a, I>(
    mut new_state: OrdMap<LedgerKey, LedgerEntry>,
    ledger_info: &LedgerInfo,
    storage_map: I,
    output_file: &std::path::PathBuf,
) -> Result<(), Error>
where
    I: IntoIterator<Item = (&'a Box<LedgerKey>, &'a Option<Box<LedgerEntry>>)>,
{
    // Need to start off with the existing snapshot (new_state) since it's
    // possible the storage_map did not touch every existing entry
    if let Some(dir) = output_file.parent() {
        if !dir.exists() {
            create_dir_all(dir)?;
        }
    }

    let file = File::create(output_file)?;
    for (lk, ole) in storage_map {
        if let Some(le) = ole {
            new_state.insert(*lk.clone(), *(*le).clone());
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
        network_passphrase: ledger_info.network_passphrase.clone(),
        base_reserve: ledger_info.base_reserve,
    };
    serde_json::to_writer(&file, &output)?;

    Ok(())
}

/// Returns a list of events from the on-disk event store, which stores events
/// exactly as they'd be returned by an RPC server.
pub fn read_events(path: &std::path::PathBuf) -> Result<Vec<rpc::Event>, Error> {
    let reader = std::fs::OpenOptions::new().read(true).open(path)?;
    let events: rpc::GetEventsResponse = serde_json::from_reader(reader)?;

    Ok(events.events)
}

// Reads the existing event file, appends the new events, and writes it all to
// disk. Note that this almost certainly isn't safe to call in parallel.
pub fn commit_events(
    new_events: &[events::HostEvent],
    ledger_info: &LedgerInfo,
    output_file: &std::path::PathBuf,
) -> Result<(), Error> {
    // Create the directory tree if necessary, since these are unlikely to be
    // the first events.
    if let Some(dir) = output_file.parent() {
        if !dir.exists() {
            create_dir_all(dir)?;
        }
    }

    let mut file = std::fs::OpenOptions::new().read(true).open(output_file)?;
    let mut events: rpc::GetEventsResponse = serde_json::from_reader(&mut file)?;

    for event in new_events.iter() {
        let contract_event = match event {
            events::HostEvent::Contract(e) => e,
            events::HostEvent::Debug(_e) => todo!(),
        };

        // TODO: Handle decoding errors cleanly; I miss errors.Wrap(err, ...) :(
        let topics = match &contract_event.body {
            xdr::ContractEventBody::V0(e) => &e.topics,
        }
        .iter()
        .map(|t| t.to_xdr_base64().unwrap())
        .collect(); // try_collect()? would be nice here

        let cereal_event = rpc::Event {
            event_type: "contract".to_string(),
            id: String::new(),
            paging_token: String::new(),
            ledger: ledger_info.sequence_number.to_string(),
            ledger_closed_at: ledger_info.timestamp.to_string(),
            contract_id: hex::encode(
                contract_event
                    .contract_id
                    .as_ref()
                    .unwrap_or(&xdr::Hash([0; 32])),
            ),
            topic: topics,
            value: rpc::EventValue {
                xdr: match &contract_event.body {
                    xdr::ContractEventBody::V0(e) => &e.data,
                }
                .to_xdr_base64()?,
            },
        };

        events.events.push(cereal_event);
    }

    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(output_file)?;

    serde_json::to_writer_pretty(&mut file, &events)?;

    Ok(())
}
