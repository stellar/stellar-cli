use clap::{ArgEnum, Parser};
use hex;
use termcolor::{Color, ColorChoice, StandardStream, WriteColor};
use termcolor_output::colored;

use soroban_env_host::xdr::{self, ReadXdr};

use crate::rpc::{Client, Event};
use crate::{rpc, snapshot, utils};
use crate::{HEADING_RPC, HEADING_SANDBOX};

#[derive(Parser, Debug)]
#[clap()]
pub struct Cmd {
    /// The first ledger sequence number in the range to pull events
    #[clap(short, long)]
    start_ledger: u32,

    /// The last (and inclusive) ledger sequence number in the range to pull events
    /// https://developers.stellar.org/docs/encyclopedia/ledger-headers#ledger-sequence
    #[clap(short, long)]
    end_ledger: u32,

    /// Formatting options for outputted events
    #[clap(long, arg_enum, default_value = "pretty")]
    format: OutputFormat,

    /// RPC server endpoint
    #[clap(long,
        env = "SOROBAN_RPC_URL",
        help_heading = HEADING_RPC,
        conflicts_with = "ledger-file", 
        conflicts_with = "events-file",
        conflicts_with = "follow",
    )]
    rpc_url: Option<String>,

    /// Local event store (likely generated by `invoke`) to pull events from
    #[clap(
        long,
        parse(from_os_str),
        value_name = "PATH",
        env = "SOROBAN_EVENTS_FILE",
        help_heading = HEADING_SANDBOX,
        conflicts_with = "rpc-url",
    )]
    events_file: Option<std::path::PathBuf>,

    /// Local sandboxed ledger file
    #[clap(
        long,
        parse(from_os_str),
        value_name = "PATH",
        env = "SOROBAN_LEDGER_FILE",
        help_heading = HEADING_SANDBOX,
        conflicts_with = "rpc-url",
    )]
    ledger_file: Option<std::path::PathBuf>,

    /// Whether or not we should keep watching the events file and displaying
    /// them in real time.
    #[clap(
        short, long,
        help_heading = HEADING_SANDBOX,
        conflicts_with = "rpc-url",
    )]
    follow: bool,

    /// A set of (up to 5) contract IDs to filter events on
    #[clap(long = "id", multiple = true, max_values(5), help_heading = "FILTERS")]
    contract_ids: Vec<String>,

    /// A set of (up to 5) topic filters to filter events on
    #[clap(
        long = "topic",
        multiple = true,
        max_values(5),
        help_heading = "FILTERS"
    )]
    topics: Vec<String>,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("invalid ledger range: low bigger than high ({low} > {high})")]
    InvalidLedgerRange { low: u32, high: u32 },

    #[error("filepath does not exist: {path}")]
    InvalidFile { path: String },

    #[error("filepath ({path}) cannot be read: {error}")]
    CannotReadFile { path: String, error: String },

    #[error("cannot parse contract ID {contract_id}: {error}")]
    InvalidContractId {
        contract_id: String,
        error: hex::FromHexError,
    },

    #[error("invalid JSON string: {error} ({debug})")]
    InvalidJson {
        debug: String,
        error: serde_json::Error,
    },

    #[error("you must specify either an RPC server or sandbox filepath(s)")]
    TargetRequired,

    #[error(transparent)]
    Rpc(#[from] rpc::Error),

    #[error(transparent)]
    Generic(#[from] Box<dyn std::error::Error>),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, ArgEnum)]
pub enum OutputFormat {
    /// Colorful, human-oriented console output
    Pretty,
    /// Human-oriented console output without colors
    Plain,
    /// JSONified console output
    Json,
}

impl Cmd {
    pub async fn run(&self, _matches: &clap::ArgMatches) -> Result<(), Error> {
        if self.start_ledger > self.end_ledger {
            return Err(Error::InvalidLedgerRange {
                low: self.start_ledger,
                high: self.end_ledger,
            });
        }

        for raw_contract_id in &self.contract_ids {
            // We parse the contract IDs to ensure they're the correct format,
            // but since we'll be passing them as-is to the RPC server anyway,
            // we disregard the return value.
            utils::contract_id_from_str(raw_contract_id).map_err(|e| Error::InvalidContractId {
                contract_id: raw_contract_id.clone(),
                error: e,
            })?;
        }

        let mut events: Vec<Event> = Vec::new();
        if let Some(rpc_url) = self.rpc_url.as_ref() {
            let client = Client::new(rpc_url);
            let rpc_event = client.get_events(&self.contract_ids, &self.topics)?;
            events = rpc_event.events;
        } else if let Some(path) = self.events_file.as_ref() {
            if !path.exists() {
                return Err(Error::InvalidFile {
                    path: path.to_str().unwrap().to_string(),
                });
            }

            // Read the JSON events from disk and find the ones that match the
            // contract ID filter(s) that were passed in.
            events.extend(
                snapshot::read_events(path)
                    .map_err(|err| Error::CannotReadFile {
                        path: path.to_str().unwrap().to_string(),
                        error: err.to_string(),
                    })?
                    .iter()
                    .filter_map(|evt| {
                        // Contract ID filter(s) are optional, so we should render
                        // all events if they're omitted.
                        if self.contract_ids.len() > 0 {
                            for id in &self.contract_ids {
                                if evt.contract_id == *id {
                                    return Some(evt);
                                }
                            }
                        } else {
                            return Some(evt);
                        }

                        None
                    })
                    .cloned()
                    .collect::<Vec<Event>>(),
            );
        } else {
            return Err(Error::TargetRequired);
        }

        for event in &events {
            match self.format {
                // Should we pretty-print the JSON like we're doing here or just
                // dump an event in raw JSON on each line? The latter is easier
                // to consume programmatically.
                OutputFormat::Json => {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&event).map_err(|e| {
                            Error::InvalidJson {
                                debug: format!("{:#?}", event),
                                error: e,
                            }
                        })?,
                    );
                }
                OutputFormat::Plain => print_event(event)?,
                OutputFormat::Pretty => pretty_print_event(event)?,
            }
        }

        Ok(())
    }
}

pub fn print_event(event: &Event) -> Result<(), Box<dyn std::error::Error>> {
    println!("Event {}:", event.id);
    println!("  Type:     {}", event.event_type);
    println!(
        "  Ledger:   {} (closed at {})",
        event.ledger, event.ledger_closed_at
    );
    println!("  Contract: {}", event.contract_id);
    println!("  Topics:");
    for topic in &event.topic {
        let scval = xdr::ScVal::from_xdr_base64(topic)?;
        println!("            {:?}", scval);
    }
    let scval = xdr::ScVal::from_xdr_base64(&event.value.xdr)?;
    println!("  Value:    {:?}", scval);

    Ok(())
}

pub fn pretty_print_event(event: &Event) -> Result<(), Box<dyn std::error::Error>> {
    let mut stdout = StandardStream::stdout(ColorChoice::Auto);
    if !stdout.supports_color() {
        print_event(event)?;
        return Ok(());
    }

    let color = match event.event_type.as_str() {
        "system" => Color::Yellow,
        _ => Color::Green,
    };
    colored!(
        stdout,
        "{}Event{} {}{}{} [{}{}{}{}]:\n",
        bold!(true),
        bold!(false),
        fg!(Some(Color::Green)),
        event.paging_token,
        reset!(),
        bold!(true),
        fg!(Some(color)),
        event.event_type.to_ascii_uppercase(),
        reset!(),
    )?;

    colored!(
        stdout,
        "  Ledger:   {}{}{} (closed at {}{}{})\n",
        fg!(Some(Color::Green)),
        event.ledger,
        reset!(),
        fg!(Some(Color::Green)),
        event.ledger_closed_at,
        reset!(),
    )?;

    colored!(
        stdout,
        "  Contract: {}0x{}{}\n",
        fg!(Some(Color::Green)),
        event.contract_id,
        reset!(),
    )?;

    colored!(stdout, "  Topics:\n")?;
    for topic in &event.topic {
        let scval = xdr::ScVal::from_xdr_base64(topic)?;
        colored!(
            stdout,
            "            {}{:?}{}\n",
            fg!(Some(Color::Green)),
            scval,
            reset!(),
        )?;
    }

    let scval = xdr::ScVal::from_xdr_base64(&event.value.xdr)?;
    colored!(
        stdout,
        "  Value: {}{:?}{}\n",
        fg!(Some(Color::Green)),
        scval,
        reset!(),
    )?;

    Ok(())
}
