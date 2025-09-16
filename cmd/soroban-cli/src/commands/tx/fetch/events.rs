use crate::{commands::global, xdr};
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
    JsonFormatted,
    /// Human readable event output with parsed XDRs
    #[default]
    Text,
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let resp = self.args.fetch_transaction(global_args).await?;
        let events = &resp.events;
        let contract_events: &Vec<Vec<xdr::ContractEvent>> = &events.contract_events;
        let diagnostic_events = &events.diagnostic_events;
        let transaction_events = &events.transaction_events;
        match self.output {
            EventsOutputFormat::Text => {
                args::Args::print_tx_summary(&resp);
                Self::print_contract_events(contract_events);
                Self::print_transaction_events(transaction_events);
                Self::print_diagnostic_events(diagnostic_events);
            }
            EventsOutputFormat::JsonFormatted => {
                args::Args::print_tx_summary(&resp);
                println!("{}", serde_json::to_string_pretty(&events)?);
            }
            EventsOutputFormat::Json => {
                println!("{}", serde_json::to_string(&events)?);
            }
        }
        Ok(())
    }

    fn get_sc_val_string(val: &xdr::ScVal) -> String {
        match val {
            xdr::ScVal::Symbol(sym) => {
                format!("Symbol: {:?}", sym.to_string())
            }
            xdr::ScVal::Address(addr) => {
                format!("Address: {:?}", addr.to_string())
            }
            xdr::ScVal::I128(val) => {
                format!("I128: {:?}", val.to_string())
            }
            other => {
                format!("Other: {other:?}")
            }
        }
    }

    fn print_contract_event(event: &xdr::ContractEvent) {
        if let Some(id) = event.contract_id.as_ref() {
            println!("  Contract Id: {id}");
        }

        match &event.body {
            xdr::ContractEventBody::V0(body) => {
                for (i, topic) in body.topics.iter().enumerate() {
                    println!("  Topic[{i}]: {}", Self::get_sc_val_string(topic));
                }
                println!("  Data: {}", Self::get_sc_val_string(&body.data));
            }
        }
    }

    fn print_contract_events(events: &[Vec<xdr::ContractEvent>]) {
        if events.is_empty() {
            println!("Contract Events: None");
            return;
        }
        println!("Contract Events:");
        for event in events.iter().flatten() {
            Self::print_contract_event(event);
            println!();
        }
    }

    fn print_transaction_events(events: &Vec<xdr::TransactionEvent>) {
        if events.is_empty() {
            println!("Transaction Events: None");
            return;
        }
        println!("Transaction Events:");
        for event in events {
            println!("  Transaction State: {:?}", event.stage);
            Self::print_contract_event(&event.event);
            println!();
        }
    }

    fn print_diagnostic_events(events: &Vec<xdr::DiagnosticEvent>) {
        if events.is_empty() {
            println!("Diagnostic Events: None");
            return;
        }

        println!("Diagnostic Events:");
        for event in events {
            println!(
                "  In Successful Contract Call: {:?}",
                event.in_successful_contract_call
            );
            Self::print_contract_event(&event.event);
            println!();
        }
    }
}
