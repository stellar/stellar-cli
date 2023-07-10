use clap::{arg, command, Parser};
use std::io;

use soroban_env_host::xdr::{self, ReadXdr};

use super::config::{events_file, locator, network};
use crate::{rpc, toid, utils};

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    /// The first ledger sequence number in the range to pull events (required
    /// if not in sandbox mode).
    /// https://developers.stellar.org/docs/encyclopedia/ledger-headers#ledger-sequence
    #[arg(long, conflicts_with = "cursor", required_unless_present = "cursor")]
    start_ledger: Option<u32>,

    /// The cursor corresponding to the start of the event range.
    #[arg(
        long,
        conflicts_with = "start_ledger",
        required_unless_present = "start_ledger"
    )]
    cursor: Option<String>,

    /// Output formatting options for event stream
    #[arg(long, value_enum, default_value = "pretty")]
    output: OutputFormat,

    /// The maximum number of events to display (specify "0" to show all events
    /// when using sandbox, or to defer to the server-defined limit if using
    /// RPC).
    #[arg(short, long, default_value = "10")]
    count: usize,

    /// A set of (up to 5) contract IDs to filter events on. This parameter can
    /// be passed multiple times, e.g. `--id abc --id def`, or passed with
    /// multiple parameters, e.g. `--id abd def`.
    ///
    /// Though the specification supports multiple filter objects (i.e.
    /// combinations of type, IDs, and topics), only one set can be specified on
    /// the command-line today, though that set can have multiple IDs/topics.
    #[arg(
        long = "id",
        num_args = 1..=6,
        help_heading = "FILTERS"
    )]
    contract_ids: Vec<String>,

    /// A set of (up to 4) topic filters to filter event topics on. A single
    /// topic filter can contain 1-4 different segment filters, separated by
    /// commas, with an asterisk (* character) indicating a wildcard segment.
    ///
    /// For example, this is one topic filter with two segments:
    ///
    ///     --topic "AAAABQAAAAdDT1VOVEVSAA==,*"
    ///
    /// This is two topic filters with one and two segments each:
    ///
    ///     --topic "AAAABQAAAAdDT1VOVEVSAA==" --topic '*,*'
    ///
    /// Note that all of these topic filters are combined with the contract IDs
    /// into a single filter (i.e. combination of type, IDs, and topics).
    #[arg(
        long = "topic",
        num_args = 1..=5,
        help_heading = "FILTERS"
    )]
    topic_filters: Vec<String>,

    /// Specifies which type of contract events to display.
    #[arg(
        long = "type",
        value_enum,
        default_value = "all",
        help_heading = "FILTERS"
    )]
    event_type: rpc::EventType,

    #[command(flatten)]
    locator: locator::Args,

    #[command(flatten)]
    network: network::Args,

    #[command(flatten)]
    events_file: events_file::Args,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("cursor is not valid")]
    InvalidCursor,

    #[error("filepath does not exist: {path}")]
    InvalidFile { path: String },

    #[error("filepath ({path}) cannot be read: {error}")]
    CannotReadFile { path: String, error: String },

    #[error("cannot parse topic filter {topic} into 1-4 segments")]
    InvalidTopicFilter { topic: String },

    #[error("invalid segment ({segment}) in topic filter ({topic}): {error}")]
    InvalidSegment {
        topic: String,
        segment: String,
        error: xdr::Error,
    },

    #[error("cannot parse contract ID {contract_id}: {error}")]
    InvalidContractId {
        contract_id: String,
        error: stellar_strkey::DecodeError,
    },

    #[error("invalid JSON string: {error} ({debug})")]
    InvalidJson {
        debug: String,
        error: serde_json::Error,
    },

    #[error("invalid timestamp in event: {ts}")]
    InvalidTimestamp { ts: String },

    #[error("missing start_ledger and cursor")]
    MissingStartLedgerAndCursor,
    #[error("missing target")]
    MissingTarget,

    #[error(transparent)]
    Rpc(#[from] rpc::Error),

    #[error(transparent)]
    Generic(#[from] Box<dyn std::error::Error>),

    #[error(transparent)]
    Io(#[from] io::Error),

    #[error(transparent)]
    Xdr(#[from] xdr::Error),

    #[error(transparent)]
    Serde(#[from] serde_json::Error),

    #[error(transparent)]
    Network(#[from] network::Error),

    #[error(transparent)]
    EventsFile(#[from] events_file::Error),

    #[error(transparent)]
    Locator(#[from] locator::Error),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, clap::ValueEnum)]
pub enum OutputFormat {
    /// Colorful, human-oriented console output
    Pretty,
    /// Human-oriented console output without colors
    Plain,
    /// JSONified console output
    Json,
}

impl Cmd {
    pub async fn run(&mut self) -> Result<(), Error> {
        // Validate that topics are made up of segments.
        for topic in &self.topic_filters {
            for (i, segment) in topic.split(',').enumerate() {
                if i > 4 {
                    return Err(Error::InvalidTopicFilter {
                        topic: topic.to_string(),
                    });
                }

                if segment != "*" {
                    if let Err(e) = xdr::ScVal::from_xdr_base64(segment) {
                        return Err(Error::InvalidSegment {
                            topic: topic.to_string(),
                            segment: segment.to_string(),
                            error: e,
                        });
                    }
                }
            }
        }

        // Validate and normalize contract_ids
        for id in &mut self.contract_ids {
            // We parse the contract IDs to ensure they're the correct format, and padded out
            // correctly.
            //
            // TODO: Once soroban-rpc supports passing these as a strkey, we should change to
            // formatting these as C-strkeys.
            *id = utils::contract_id_from_str(id)
                .map(hex::encode)
                .map_err(|e| Error::InvalidContractId {
                    contract_id: id.clone(),
                    error: e,
                })?;
        }

        let response = if self.network.is_no_network() {
            self.run_in_sandbox()
        } else {
            self.run_against_rpc_server().await
        }?;

        for event in &response.events {
            match self.output {
                // Should we pretty-print the JSON like we're doing here or just
                // dump an event in raw JSON on each line? The latter is easier
                // to consume programmatically.
                OutputFormat::Json => {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&event).map_err(|e| {
                            Error::InvalidJson {
                                debug: format!("{event:#?}"),
                                error: e,
                            }
                        })?,
                    );
                }
                OutputFormat::Plain => println!("{event}"),
                OutputFormat::Pretty => event.pretty_print()?,
            }
        }
        println!("Latest Ledger: {}", response.latest_ledger);

        Ok(())
    }

    async fn run_against_rpc_server(&self) -> Result<rpc::GetEventsResponse, Error> {
        let start = self.start()?;
        let network = self.network.get(&self.locator)?;

        let client = rpc::Client::new(&network.rpc_url)?;
        client
            .verify_network_passphrase(Some(&network.network_passphrase))
            .await?;
        client
            .get_events(
                start,
                Some(self.event_type),
                &self.contract_ids,
                &self.topic_filters,
                Some(self.count),
            )
            .await
            .map_err(Error::Rpc)
    }

    pub fn run_in_sandbox(&self) -> Result<rpc::GetEventsResponse, Error> {
        let start = self.start()?;
        let count: usize = if self.count == 0 {
            std::usize::MAX
        } else {
            self.count
        };

        let start_cursor = match start {
            rpc::EventStart::Ledger(l) => (toid::Toid::new(l, 0, 0).into(), -1),
            rpc::EventStart::Cursor(c) => rpc::parse_cursor(&c)?,
        };
        let path = self.locator.config_dir()?;
        let file = self.events_file.read(&path)?;

        // Read the JSON events from disk and find the ones that match the
        // contract ID filter(s) that were passed in.
        Ok(rpc::GetEventsResponse {
            events: events_file::Args::filter_events(
                &file.events,
                &path,
                start_cursor,
                &self.contract_ids,
                &self.topic_filters,
                count,
            ),
            latest_ledger: file.latest_ledger,
        })
    }

    fn start(&self) -> Result<rpc::EventStart, Error> {
        let start = match (self.start_ledger, self.cursor.clone()) {
            (Some(start), _) => rpc::EventStart::Ledger(start),
            (_, Some(c)) => rpc::EventStart::Cursor(c),
            // should never happen because of required_unless_present flags
            _ => return Err(Error::MissingStartLedgerAndCursor),
        };
        Ok(start)
    }
}

#[cfg(test)]
mod tests {
    use std::path;

    use assert_fs::NamedTempFile;
    use soroban_env_host::events;

    use super::*;

    use events_file::Args;
    #[test]
    fn test_does_event_serialization_match() {
        let temp = NamedTempFile::new("events.json").unwrap();
        let events_file = Args {
            events_file: Some(temp.to_path_buf()),
        };
        // Make a couple of fake events with slightly different properties and
        // write them to disk, then read the serialized versions from disk and
        // ensure the properties match.

        let events: Vec<events::HostEvent> = vec![
            events::HostEvent {
                event: xdr::ContractEvent {
                    ext: xdr::ExtensionPoint::V0,
                    contract_id: Some(xdr::Hash([0; 32])),
                    type_: xdr::ContractEventType::Contract,
                    body: xdr::ContractEventBody::V0(xdr::ContractEventV0 {
                        topics: xdr::ScVec(vec![].try_into().unwrap()),
                        data: xdr::ScVal::U32(12345),
                    }),
                },
                failed_call: false,
            },
            events::HostEvent {
                event: xdr::ContractEvent {
                    ext: xdr::ExtensionPoint::V0,
                    contract_id: Some(xdr::Hash([0x1; 32])),
                    type_: xdr::ContractEventType::Contract,
                    body: xdr::ContractEventBody::V0(xdr::ContractEventV0 {
                        topics: xdr::ScVec(vec![].try_into().unwrap()),
                        data: xdr::ScVal::I32(67890),
                    }),
                },
                failed_call: false,
            },
        ];

        let ledger_info = soroban_ledger_snapshot::LedgerSnapshot {
            protocol_version: 1,
            sequence_number: 2, // this is the only value that matters
            timestamp: 3,
            network_id: [0x1; 32],
            base_reserve: 5,
            ledger_entries: vec![],
            max_entry_expiration: 6,
            min_persistent_entry_expiration: 7,
            min_temp_entry_expiration: 8,
        };

        events_file.commit(&events, &ledger_info, &temp).unwrap();

        let file = events_file.read(&std::env::current_dir().unwrap()).unwrap();
        assert_eq!(file.events.len(), 2);
        assert_eq!(file.events[0].ledger, "2");
        assert_eq!(file.events[1].ledger, "2");
        assert_eq!(file.events[0].contract_id, "0".repeat(64));
        assert_eq!(file.events[1].contract_id, "01".repeat(32));
        assert_eq!(file.latest_ledger, 2);
    }

    #[test]
    fn test_does_event_fixture_load() {
        // This test ensures that the included JSON fixture file matches the
        // correct event format (for the purposes of human readability).
        let filename =
            path::PathBuf::from("../crates/soroban-test/tests/fixtures/test-jsons/get-events.json");
        let events_file = Args {
            events_file: Some(filename),
        };
        let result = events_file.read(&std::env::current_dir().unwrap());
        println!("{result:?}");
        assert!(result.is_ok());
    }
}
