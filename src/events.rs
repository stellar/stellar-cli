use clap::Parser;
use hex::FromHexError;

use crate::utils;
use crate::HEADING_RPC;
use crate::{rpc, rpc::Client};

#[derive(Parser, Debug)]
#[clap()]
pub struct Cmd {
    /// The ledger range to pull events from
    start_ledger: u32,
    end_ledger: u32,

    /// A set of (up to 5) contract IDs to filter events on
    #[clap(long = "ids", multiple = true)]
    contract_ids: Vec<String>,

    /// A set of (up to 5) topic filters to filter events on
    #[clap(long, multiple = true)]
    topics: Vec<String>,

    /// RPC server endpoint
    #[clap(long, env = "SOROBAN_RPC_URL", help_heading = HEADING_RPC)]
    rpc_url: Option<String>,
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
}

impl Cmd {
    pub async fn run(&self, _matches: &clap::ArgMatches) -> Result<(), Error> {
        println!("{:#?}", self.contract_ids);
        if self.contract_ids.len() > 5 {
            return Err(Error::TooManyContractIds {});
        }

        println!("{:#?}", self.topics);
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

        let mut topics: Vec<String> = Vec::new();
        for topic in self.topics.iter() {
            // if topic == "*" || topic == "#" {
            topics.push(topic.clone());
            // } else {
            //     // Like with IDs, we just ensure that the segments are valid
            //     match ScVal::from_xdr_base64(topic.clone())? {
            //         ScVal::Object(_) => {}
            //     }
            // }
        }

        if self.rpc_url.is_some() {
            let client = Client::new(self.rpc_url.as_ref().unwrap());
            let events = client.get_events(&self.contract_ids, &topics).await?;

            for event in events.events.iter() {
                println!("{:#?}", event);
            }
        }

        Ok(())
    }
}
