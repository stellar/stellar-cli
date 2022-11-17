use std::io;
use std::io::Write;

use clap::Parser;
use hex::FromHexError;
use soroban_env_host::xdr::{ReadXdr, ScVal, WriteXdr};
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

use crate::rpc::Event;
use crate::utils;
use crate::{rpc, rpc::Client};
use crate::{HEADING_RPC, HEADING_SANDBOX};

#[derive(Parser, Debug)]
#[clap()]
pub struct Cmd {
    /// The ledger range to pull events from
    start_ledger: u32,
    end_ledger: u32,

    /// RPC server endpoint
    #[clap(long,
        env = "SOROBAN_RPC_URL",
        help_heading = HEADING_RPC,
        conflicts_with = "sandbox",
    )]
    rpc_url: Option<String>,

    /// Sandbox file from which to pull events from
    #[clap(
        long,
        parse(from_os_str),
        value_name = "PATH",
        env = "SOROBAN_LEDGER_FILE",
        help_heading = HEADING_SANDBOX,
        conflicts_with = "rpc-url",
    )]
    sandbox: Option<std::path::PathBuf>,

    /// A set of (up to 5) contract IDs to filter events on
    #[clap(long = "ids", multiple = true)]
    contract_ids: Vec<String>,

    /// A set of (up to 5) topic filters to filter events on
    #[clap(long, multiple = true)]
    topics: Vec<String>,

    /// Formatting options: either console or json.
    #[clap(
        long, 
        possible_values = ["console", "json"],
        default_value = "console"
    )]
    format: String,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("invalid ledger range: low bigger than high ({low} > {high})")]
    InvalidLedgerRange { low: u32, high: u32 },
    
    #[error("cannot parse contract ID {contract_id}: {error}")]
    CannotParseContractId {
        contract_id: String,
        error: FromHexError,
    },
    
    #[error("too many contracts IDs (max 5)")]
    TooManyContractIds {},
    
    #[error("too many topic filters (max 5)")]
    TooManyTopicFilters {},

    #[error(transparent)]
    Rpc(#[from] rpc::Error),
    
    #[error(transparent)]
    Generic(#[from] Box<dyn std::error::Error>),
}

impl Cmd {
    pub async fn run(&self, _matches: &clap::ArgMatches) -> Result<(), Error> {
        if self.contract_ids.len() > 5 {
            return Err(Error::TooManyContractIds {});
        }

        if self.topics.len() > 5 {
            return Err(Error::TooManyTopicFilters {});
        }

        if self.start_ledger > self.end_ledger {
            return Err(Error::InvalidLedgerRange {
                low: self.start_ledger,
                high: self.end_ledger,
            });
        }

        for raw_contract_id in self.contract_ids.iter() {
            // We parse the contract IDs to ensure they're the correct format,
            // but since we'll be passing them as-is to the RPC server anyway,
            // we disregard the return value.
            utils::contract_id_from_str(&raw_contract_id).map_err(|e| {
                Error::CannotParseContractId {
                    contract_id: raw_contract_id.clone(),
                    error: e,
                }
            })?;
        }

        let mut events: Vec<Event> = Vec::new();
        if self.rpc_url.is_some() {
            let client = Client::new(self.rpc_url.as_ref().unwrap());
            let rpc_event = client.get_events(&self.contract_ids, &self.topics).await?;
            events = rpc_event.events;
        } else if self.sandbox.is_some() {
            // TODO: Get events from the sandbox.
            let path = self.sandbox.as_ref().unwrap();
            if !path.exists() {
                panic!(
                    "Provided path ({}) does not exist",
                    path.to_str().unwrap()
                );
            }
        }

        for event in events.iter() {
            if self.format == "json" {
                println!("{}", serde_json::to_string_pretty(&event).unwrap());
            } else {
                print_event_in_color(event)?;
            }
        }

        Ok(())
    }
}

pub fn print_event_in_color(event: &Event) -> Result<(), Box<dyn std::error::Error>> {
    let mut stdout = StandardStream::stdout(ColorChoice::Auto);

    set_white(&mut stdout)?;
    write!(&mut stdout, "Event ")?;
    set_green(&mut stdout)?;
    write!(&mut stdout, "{}", event.id)?;

    set_white(&mut stdout)?;
    write!(&mut stdout, ":\n  Ledger:   ")?;
    set_green(&mut stdout)?;
    write!(&mut stdout, "{}", event.ledger)?;
    set_white(&mut stdout)?;
    write!(&mut stdout, " (closed at ")?;
    set_green(&mut stdout)?;
    write!(&mut stdout, "{}", event.ledger_closed_at)?;

    set_white(&mut stdout)?;
    write!(&mut stdout, ")\n  Contract: ")?;
    set_green(&mut stdout)?;
    write!(&mut stdout, "{}", event.contract_id)?;

    set_white(&mut stdout)?;
    write!(&mut stdout, "\n  Topics:")?;
    set_green(&mut stdout)?;
    for topic in event.topic.iter() {
        let scval = ScVal::from_xdr_base64(topic)?;
        write!(&mut stdout, "\n            {:?}", scval)?;
    }
    set_white(&mut stdout)?;
    write!(&mut stdout, "\n  Value: ")?;
    set_green(&mut stdout)?;
    let scval = ScVal::from_xdr_base64(&event.value)?;
    writeln!(&mut stdout, "{:?}", scval)?;

    set_white(&mut stdout)?;
    Ok(())
}

fn set_green(ss: &mut StandardStream) -> io::Result<()> {
    ss.set_color(ColorSpec::new().set_fg(Some(Color::Green)))?;
    write!(ss, "")
}

fn set_white(ss: &mut StandardStream) -> io::Result<()> {
    ss.set_color(ColorSpec::new().set_fg(None))?;
    write!(ss, "")
}
