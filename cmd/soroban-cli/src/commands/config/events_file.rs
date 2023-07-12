use crate::{
    commands::HEADING_SANDBOX,
    rpc::{self, does_topic_match, Event},
    toid,
};
use chrono::{DateTime, NaiveDateTime, Utc};
use clap::arg;
use soroban_env_host::{
    events,
    xdr::{self, WriteXdr},
};
use soroban_ledger_snapshot::LedgerSnapshot;
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, clap::Args, Clone, Default)]
#[group(skip)]
pub struct Args {
    /// File to persist events, default is `.soroban/events.json`
    #[arg(
        long,
        value_name = "PATH",
        env = "SOROBAN_EVENTS_FILE",
        help_heading = HEADING_SANDBOX,
        conflicts_with = "rpc_url",
        conflicts_with = "network",
    )]
    pub events_file: Option<PathBuf>,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Xdr(#[from] xdr::Error),
    #[error(transparent)]
    Rpc(#[from] rpc::Error),
    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Generic(#[from] Box<dyn std::error::Error>),
    #[error("invalid timestamp in event: {ts}")]
    InvalidTimestamp { ts: String },
}

impl Args {
    /// Returns a list of events from the on-disk event store, which stores events
    /// exactly as they'd be returned by an RPC server.
    pub fn read(&self, pwd: &Path) -> Result<rpc::GetEventsResponse, Error> {
        let path = self.path(pwd);
        let reader = std::fs::OpenOptions::new().read(true).open(path)?;
        Ok(serde_json::from_reader(reader)?)
    }

    /// Reads the existing event file, appends the new events, and writes it all to
    /// disk. Note that this almost certainly isn't safe to call in parallel.
    pub fn commit(
        &self,
        new_events: &[events::HostEvent],
        ledger_info: &LedgerSnapshot,
        pwd: &Path,
    ) -> Result<(), Error> {
        let output_file = self.path(pwd);
        // Create the directory tree if necessary, since these are unlikely to be
        // the first events.
        if let Some(dir) = output_file.parent() {
            if !dir.exists() {
                fs::create_dir_all(dir)?;
            }
        }

        let mut events: Vec<rpc::Event> = if output_file.exists() {
            let mut file = fs::OpenOptions::new().read(true).open(&output_file)?;
            let payload: rpc::GetEventsResponse = serde_json::from_reader(&mut file)?;
            payload.events
        } else {
            vec![]
        };

        for (i, event) in new_events.iter().enumerate() {
            let contract_event = &event.event;
            let topic = match &contract_event.body {
                xdr::ContractEventBody::V0(e) => &e.topics,
            }
            .iter()
            .map(xdr::WriteXdr::to_xdr_base64)
            .collect::<Result<Vec<String>, _>>()?;

            // stolen from
            // https://github.com/stellar/soroban-tools/blob/main/cmd/soroban-rpc/internal/methods/get_events.go#L264
            let id = format!(
                "{}-{:010}",
                toid::Toid::new(
                    ledger_info.sequence_number,
                    // we should technically inject the tx order here from the
                    // ledger info, but the sandbox does one tx/op per ledger
                    // anyway, so this is a safe assumption
                    1,
                    1,
                )
                .to_paging_token(),
                i + 1
            );

            // Misc. timestamp to RFC 3339-formatted datetime nonsense, with an
            // absurd amount of verbosity because every edge case needs its own
            // chain of error-handling methods.
            //
            // Reference: https://stackoverflow.com/a/50072164
            let ts: i64 =
                ledger_info
                    .timestamp
                    .try_into()
                    .map_err(|_e| Error::InvalidTimestamp {
                        ts: ledger_info.timestamp.to_string(),
                    })?;
            let ndt = NaiveDateTime::from_timestamp_opt(ts, 0).ok_or_else(|| {
                Error::InvalidTimestamp {
                    ts: ledger_info.timestamp.to_string(),
                }
            })?;

            let dt: DateTime<Utc> = DateTime::from_utc(ndt, Utc);

            let cereal_event = rpc::Event {
                event_type: match contract_event.type_ {
                    xdr::ContractEventType::Contract => "contract",
                    xdr::ContractEventType::System => "system",
                    xdr::ContractEventType::Diagnostic => "diagnostic",
                }
                .to_string(),
                paging_token: id.clone(),
                id,
                ledger: ledger_info.sequence_number.to_string(),
                ledger_closed_at: dt.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
                contract_id: hex::encode(
                    contract_event
                        .contract_id
                        .as_ref()
                        .unwrap_or(&xdr::Hash([0; 32])),
                ),
                topic,
                value: rpc::EventValue {
                    xdr: match &contract_event.body {
                        xdr::ContractEventBody::V0(e) => &e.data,
                    }
                    .to_xdr_base64()?,
                },
            };

            events.push(cereal_event);
        }

        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&output_file)?;

        serde_json::to_writer_pretty(
            &mut file,
            &rpc::GetEventsResponse {
                events,
                latest_ledger: ledger_info.sequence_number,
            },
        )?;

        Ok(())
    }

    pub fn path(&self, pwd: &Path) -> PathBuf {
        if let Some(path) = &self.events_file {
            path.clone()
        } else {
            pwd.join("events.json")
        }
    }

    pub fn filter_events(
        events: &[Event],
        path: &Path,
        start_cursor: (u64, i32),
        contract_ids: &[String],
        topic_filters: &[String],
        count: usize,
    ) -> Vec<Event> {
        events
            .iter()
            .filter(|evt| match evt.parse_cursor() {
                Ok(event_cursor) => event_cursor > start_cursor,
                Err(e) => {
                    eprintln!("error parsing key 'ledger': {e:?}");
                    eprintln!(
                        "your sandbox events file ('{path:?}') may be corrupt, consider deleting it",
                    );
                    eprintln!("ignoring this event: {evt:#?}");

                    false
                }
            })
            .filter(|evt| {
                // Contract ID filter(s) are optional, so we should render all
                // events if they're omitted.
                contract_ids.is_empty() || contract_ids.iter().any(|id| *id == evt.contract_id)
            })
            .filter(|evt| {
                // Like before, no topic filters means pass everything through.
                topic_filters.is_empty() ||
                // Reminder: All of the topic filters are part of a single
                // filter object, and each one contains segments, so we need to
                // apply all of them to the given event.
                topic_filters
                    .iter()
                    // quadratic, but both are <= 5 long
                    .any(|f| {
                        does_topic_match(
                            &evt.topic,
                            // misc. Rust nonsense: make a copy over the given
                            // split filter, because passing a slice of
                            // references is too much for this language to
                            // handle
                            &f.split(',')
                            .map(std::string::ToString::to_string)
                            .collect::<Vec<String>>()
                        )
                    })
            })
            .take(count)
            .cloned()
            .collect()
    }
}
