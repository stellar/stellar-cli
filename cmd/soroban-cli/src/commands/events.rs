use clap::Parser;
use indexmap::IndexMap;
use soroban_spec_tools::event::DecodedEvent;
use soroban_spec_tools::Spec;
use std::collections::HashMap;
use std::io;

use crate::xdr::{self, Limits, ReadXdr, ScVal};
use crate::{
    config::{self, locator, network},
    get_spec::get_remote_contract_spec,
    rpc,
};

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[allow(clippy::doc_markdown)]
    /// The first ledger sequence number in the range to pull events
    /// https://developers.stellar.org/docs/learn/encyclopedia/network-configuration/ledger-headers#ledger-sequence
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

    /// The maximum number of events to display (defer to the server-defined limit).
    #[arg(short, long, default_value = "10")]
    count: usize,

    /// A set of (up to 5) contract IDs to filter events on. This parameter can
    /// be passed multiple times, e.g. `--id C123.. --id C456..`, or passed with
    /// multiple parameters, e.g. `--id C123 C456`.
    ///
    /// Though the specification supports multiple filter objects (i.e.
    /// combinations of type, IDs, and topics), only one set can be specified on
    /// the command-line today, though that set can have multiple IDs/topics.
    #[arg(
        long = "id",
        num_args = 1..=6,
        help_heading = "FILTERS"
    )]
    contract_ids: Vec<config::UnresolvedContract>,

    /// A set of (up to 5) topic filters to filter event topics on. A single
    /// topic filter can contain 1-4 different segments, separated by
    /// commas. An asterisk (`*` character) indicates a wildcard segment.
    ///
    /// In addition to up to 4 possible topic filter segments, the "**" wildcard can also be added, and will allow for a flexible number of topics in the returned events. The "**" wildcard must be the last segment in a query.
    ///
    /// If the "**" wildcard is not included, only events with the exact number of topics as the given filter will be returned.
    ///
    /// **Example:** topic filter with two segments: `--topic "AAAABQAAAAdDT1VOVEVSAA==,*"`
    ///
    /// **Example:** two topic filters with one and two segments each: `--topic "AAAABQAAAAdDT1VOVEVSAA==" --topic '*,*'`
    ///
    /// **Example:** topic filter with four segments and the "**" wildcard: --topic "AAAABQAAAAdDT1VOVEVSAA==,*,*,*,**"
    ///
    /// Note that all of these topic filters are combined with the contract IDs
    /// into a single filter (i.e. combination of type, IDs, and topics).
    #[arg(
        long = "topic",
        num_args = 1.., // allowing 1+ arguments here, and doing additional validation in parse_topics
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
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("cursor is not valid")]
    InvalidCursor,
    #[error("filepath does not exist: {path}")]
    InvalidFile { path: String },
    #[error("filepath ({path}) cannot be read: {error}")]
    CannotReadFile { path: String, error: String },
    #[error("max of 5 topic filters allowed per request, received {filter_count}")]
    MaxTopicFilters { filter_count: usize },
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
    Locator(#[from] locator::Error),
    #[error(transparent)]
    Config(#[from] config::Error),
    #[error(transparent)]
    GetSpec(#[from] crate::get_spec::Error),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, clap::ValueEnum)]
pub enum OutputFormat {
    /// Human-readable output with decoded event names and parameters
    Pretty,

    /// Human-readable output without colors
    Plain,

    /// JSON output with decoded event names and parameters
    Json,

    /// Raw event output without self-describing decoding
    Raw,
}

/// Cache for contract specs, keyed by contract ID
type SpecCache = HashMap<String, Option<Spec>>;

/// Decoded event with metadata for JSON output.
///
/// This is intentionally a different schema from the raw `rpc::Event` format,
/// focused on providing decoded event data with named parameters. Key differences:
/// - `event_name`: The decoded event name from the contract spec (e.g., "Transfer")
/// - `params`: Named parameters decoded using the contract spec
///
/// For the raw event format with all original fields (topics, value as base64 XDR),
/// use `--output raw`.
#[derive(serde::Serialize, Debug)]
struct DecodedEventWithMetadata {
    id: String,
    ledger: u32,
    ledger_closed_at: String,
    #[serde(rename = "type")]
    event_type: String,
    contract_id: String,
    event_name: String,
    prefix_topics: Vec<String>,
    params: IndexMap<String, serde_json::Value>,
}

impl Cmd {
    pub async fn run(&mut self) -> Result<(), Error> {
        let config = config::Args {
            locator: self.locator.clone(),
            network: self.network.clone(),
            source_account: config::UnresolvedMuxedAccount::default(),
            sign_with: config::sign_with::Args::default(),
            fee: None,
            inclusion_fee: None,
        };
        let response = self.execute(&config).await?;

        if response.events.is_empty() {
            eprintln!("No events");
            return Ok(());
        }

        // Build spec cache for decoded output formats (not raw)
        let spec_cache = if self.output == OutputFormat::Raw {
            HashMap::new()
        } else {
            self.build_spec_cache(&response.events, &config).await
        };

        for event in &response.events {
            let decoded = if self.output == OutputFormat::Raw {
                None
            } else {
                Self::try_decode_event(event, &spec_cache)
            };

            match self.output {
                OutputFormat::Pretty => {
                    if let Some(decoded) = decoded {
                        Self::print_decoded_event(&decoded, event, true)?;
                    } else {
                        event.pretty_print()?;
                    }
                }
                OutputFormat::Plain => {
                    if let Some(decoded) = decoded {
                        Self::print_decoded_event(&decoded, event, false)?;
                    } else {
                        println!("{event}");
                    }
                }
                OutputFormat::Json => {
                    // Single-line JSON (NDJSON) for streaming processing
                    if let Some(decoded) = decoded {
                        let with_metadata = DecodedEventWithMetadata {
                            id: event.id.clone(),
                            ledger: event.ledger,
                            ledger_closed_at: event.ledger_closed_at.clone(),
                            event_type: event.event_type.clone(),
                            contract_id: decoded.contract_id.clone(),
                            event_name: decoded.event_name.clone(),
                            prefix_topics: decoded.prefix_topics.clone(),
                            params: decoded.params.clone(),
                        };
                        println!(
                            "{}",
                            serde_json::to_string(&with_metadata).map_err(|e| {
                                Error::InvalidJson {
                                    debug: format!("{with_metadata:#?}"),
                                    error: e,
                                }
                            })?
                        );
                    } else {
                        println!(
                            "{}",
                            serde_json::to_string(&event).map_err(|e| {
                                Error::InvalidJson {
                                    debug: format!("{event:#?}"),
                                    error: e,
                                }
                            })?
                        );
                    }
                }
                OutputFormat::Raw => {
                    event.pretty_print()?;
                }
            }
        }
        Ok(())
    }

    /// Build a cache of contract specs for the unique contract IDs in the events
    async fn build_spec_cache(&self, events: &[rpc::Event], config: &config::Args) -> SpecCache {
        // Collect unique contract IDs
        let unique_ids: Vec<_> = events
            .iter()
            .map(|e| e.contract_id.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        // Fetch specs concurrently
        let fetch_futures: Vec<_> = unique_ids
            .iter()
            .map(|id| Self::fetch_spec_for_contract(id, config))
            .collect();

        let results = futures::future::join_all(fetch_futures).await;

        unique_ids.into_iter().zip(results).collect()
    }

    /// Fetch the spec for a single contract, returning None on failure
    async fn fetch_spec_for_contract(contract_id_str: &str, config: &config::Args) -> Option<Spec> {
        // Parse contract ID from string
        let contract_id = match stellar_strkey::Contract::from_string(contract_id_str) {
            Ok(id) => id,
            Err(e) => {
                tracing::debug!("Failed to parse contract ID {contract_id_str}: {e}");
                return None;
            }
        };

        match get_remote_contract_spec(
            &contract_id.0,
            &config.locator,
            &config.network,
            None,
            Some(config),
        )
        .await
        {
            Ok(spec_entries) => Some(Spec::new(&spec_entries)),
            Err(e) => {
                tracing::debug!(
                    "Failed to fetch spec for contract {contract_id_str}: {e}. Events from this contract will use raw format."
                );
                None
            }
        }
    }

    /// Try to decode an event using the spec cache
    fn try_decode_event(event: &rpc::Event, spec_cache: &SpecCache) -> Option<DecodedEvent> {
        let spec = spec_cache.get(&event.contract_id)?.as_ref()?;

        // Decode topics from base64 XDR
        let topics: Vec<ScVal> = event
            .topic
            .iter()
            .filter_map(|t| ScVal::from_xdr_base64(t, Limits::none()).ok())
            .collect();

        if topics.len() != event.topic.len() {
            return None; // Failed to decode some topics
        }

        // Decode value from base64 XDR
        let data = ScVal::from_xdr_base64(&event.value, Limits::none()).ok()?;

        spec.decode_event(&event.contract_id, &topics, &data)
            .inspect_err(|e| tracing::debug!("Failed to decode event {}: {e}", event.id))
            .ok()
    }

    /// Print a decoded event (with colors if use_colors is true, auto-detected for Pretty)
    fn print_decoded_event(
        decoded: &DecodedEvent,
        event: &rpc::Event,
        use_colors: bool,
    ) -> Result<(), Error> {
        use std::io::Write;
        use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

        let color_choice = if use_colors {
            ColorChoice::Auto
        } else {
            ColorChoice::Never
        };
        let mut stdout = StandardStream::stdout(color_choice);

        // Event header
        stdout.set_color(ColorSpec::new().set_fg(Some(Color::Cyan)).set_bold(true))?;
        write!(stdout, "Event")?;
        stdout.reset()?;
        writeln!(
            stdout,
            " {} [{}]:",
            event.id,
            event.event_type.to_uppercase()
        )?;

        // Ledger info
        stdout.set_color(ColorSpec::new().set_fg(Some(Color::White)).set_dimmed(true))?;
        write!(stdout, "  Ledger:   ")?;
        stdout.reset()?;
        writeln!(
            stdout,
            "{} (closed at {})",
            event.ledger, event.ledger_closed_at
        )?;

        // Contract
        stdout.set_color(ColorSpec::new().set_fg(Some(Color::White)).set_dimmed(true))?;
        write!(stdout, "  Contract: ")?;
        stdout.reset()?;
        writeln!(stdout, "{}", decoded.contract_id)?;

        // Event name with prefix topics
        stdout.set_color(ColorSpec::new().set_fg(Some(Color::White)).set_dimmed(true))?;
        write!(stdout, "  Event:    ")?;
        stdout.reset()?;
        stdout.set_color(ColorSpec::new().set_fg(Some(Color::Green)).set_bold(true))?;
        write!(stdout, "{}", decoded.event_name)?;
        stdout.reset()?;
        if !decoded.prefix_topics.is_empty() {
            write!(stdout, " ({})", decoded.prefix_topics.join(", "))?;
        }
        writeln!(stdout)?;

        // Params
        if !decoded.params.is_empty() {
            stdout.set_color(ColorSpec::new().set_fg(Some(Color::White)).set_dimmed(true))?;
            writeln!(stdout, "  Params:")?;
            stdout.reset()?;
            for (name, value) in &decoded.params {
                stdout.set_color(ColorSpec::new().set_fg(Some(Color::Yellow)))?;
                write!(stdout, "    {name}")?;
                stdout.reset()?;
                write!(stdout, ": ")?;
                stdout.set_color(ColorSpec::new().set_fg(Some(Color::White)))?;
                writeln!(stdout, "{value}")?;
                stdout.reset()?;
            }
        }

        writeln!(stdout)?;
        Ok(())
    }

    pub async fn execute(&self, config: &config::Args) -> Result<rpc::GetEventsResponse, Error> {
        let start = self.start()?;
        let network = config.get_network()?;
        let client = network.rpc_client()?;
        client
            .verify_network_passphrase(Some(&network.network_passphrase))
            .await?;

        let contract_ids: Vec<String> = self
            .contract_ids
            .iter()
            .map(|id| {
                Ok(id
                    .resolve_contract_id(&self.locator, &network.network_passphrase)?
                    .to_string())
            })
            .collect::<Result<Vec<_>, Error>>()?;

        let parsed_topics = self.parse_topics()?;

        client
            .get_events(
                start,
                Some(self.event_type),
                &contract_ids,
                &parsed_topics,
                Some(self.count),
            )
            .await
            .map_err(Error::Rpc)
    }

    fn parse_topics(&self) -> Result<Vec<rpc::TopicFilter>, Error> {
        if self.topic_filters.len() > 5 {
            return Err(Error::MaxTopicFilters {
                filter_count: self.topic_filters.len(),
            });
        }
        let mut topic_filters: Vec<rpc::TopicFilter> = Vec::new();
        for topic in &self.topic_filters {
            let mut topic_filter: rpc::TopicFilter = Vec::new(); // a topic filter is a collection of segments
            for (i, segment) in topic.split(',').enumerate() {
                if i > 4 {
                    return Err(Error::InvalidTopicFilter {
                        topic: topic.clone(),
                    });
                }

                if segment == "*" || segment == "**" {
                    topic_filter.push(segment.to_owned());
                } else {
                    match xdr::ScVal::from_xdr_base64(segment, Limits::none()) {
                        Ok(_s) => {
                            topic_filter.push(segment.to_owned());
                        }
                        Err(e) => {
                            return Err(Error::InvalidSegment {
                                topic: topic.clone(),
                                segment: segment.to_string(),
                                error: e,
                            });
                        }
                    }
                }
            }
            topic_filters.push(topic_filter);
        }

        Ok(topic_filters)
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
