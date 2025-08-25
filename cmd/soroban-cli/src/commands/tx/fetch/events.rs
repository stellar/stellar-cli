use crate::{
    commands::global,
    xdr,
};
use clap::{command, Parser};

use super::args;

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    args: args::Args,

    /// Format of the output
    #[arg(long, default_value = "json")]
    output: EventsOutputFormat,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Serde(#[from] serde_json::Error),
    #[error(transparent)]
    Xdr(#[from] xdr::Error),
    #[error(transparent)]
    Args(#[from] args::Error),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, clap::ValueEnum, Default)]
pub enum EventsOutputFormat {
    /// JSON output of the events with parsed XDRs (one line, not formatted)
    Json,
    /// Formatted (multiline) JSON output of events with parsed XDRs
    #[default]
    JsonFormatted,
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let events = self.args.fetch_transaction(global_args).await?.events;
        match self.output {
            EventsOutputFormat::Json => {
                println!("{}", serde_json::to_string(&events)?);
            }
            EventsOutputFormat::JsonFormatted => {
                println!("{}", serde_json::to_string_pretty(&events)?);
            }
        }
        
        Ok(())
    }
}