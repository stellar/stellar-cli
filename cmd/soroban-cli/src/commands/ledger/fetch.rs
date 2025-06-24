use soroban_rpc::GetLedgersResponse;

use crate::{
    commands::global,
    config::network,
    rpc,
    xdr::{self, Hash, ReadXdr},
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Network(#[from] network::Error),
    #[error(transparent)]
    Rpc(#[from] rpc::Error),
    #[error(transparent)]
    Serde(#[from] serde_json::Error),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, clap::ValueEnum, Default)]
pub enum OutputFormat {
    /// Text output of network info
    #[default]
    Text,
    /// JSON result of the RPC request
    Json,
    /// Formatted (multiline) JSON output of the RPC request
    JsonFormatted,
}

#[derive(Debug, clap::Parser)]
pub struct Cmd {
    /// Ledger Sequence to start fetch (inclusive)
    pub seq: u32,

    /// Number of ledgers to fetch 
    #[arg(long, default_value_t=1)]
    pub limit: usize,

    #[command(flatten)]
    pub network: network::Args,

    /// Format of the output
    #[arg(long, value_enum, default_value_t)]
    pub output: OutputFormat,
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let network = self.network.get(&global_args.locator)?;
        let client = network.rpc_client()?;
        let result = client.get_ledgers(self.start(), Some(self.limit)).await?;

        match self.output {
            OutputFormat::Text => {
                let start = match self.start() {
                    soroban_rpc::LedgerStart::Ledger(l) => l.to_string(),
                    soroban_rpc::LedgerStart::Cursor(c) => c,
                };
                println!("Latest Ledger {} at close time {}", result.latest_ledger, result.latest_ledger_close_time);
                println!("Oldest Ledger {} at close time {}", result.oldest_ledger, result.oldest_ledger_close_time);
                println!("Ledgers (limit {} starting at {} )", self.limit, start);
                println!("----------------------------------------------------\n");
                for ledger in result.ledgers.clone() {
                    let ledger_header = xdr::LedgerHeaderHistoryEntry::from_xdr_base64(&ledger.header_xdr, xdr::Limits::none()).unwrap();
                    let ledger_meta = xdr::LedgerCloseMeta::from_xdr_base64(&ledger.metadata_xdr, xdr::Limits::none()).unwrap();
                    println!("Ledger Sequence: {}", ledger.sequence);
                    println!("Hash: {}", ledger.hash);
                    println!("Close Time: {}", ledger.ledger_close_time);
                    println!("Header: {}", serde_json::to_string_pretty(&ledger_header)?);
                    println!("Meta XDR: {}", serde_json::to_string_pretty(&ledger_meta)?);
                    println!("----------------------------------------------------\n");
                } 
            },
            OutputFormat::Json => println!("{}", serde_json::to_string(&result)?),
            OutputFormat::JsonFormatted => println!("{}", serde_json::to_string_pretty(&result)?),
        }

        Ok(())
    }

    fn start(&self) -> rpc::LedgerStart {
        rpc::LedgerStart::Ledger(self.seq)
        // let start = match (self.seq, self.cursor.clone()) {
        //     (Some(start), _) => rpc::EventStart::Ledger(start),
        //     (_, Some(c)) => rpc::EventStart::Cursor(c),
        //     // should never happen because of required_unless_present flags
        //     _ => return Err(Error::MissingStartLedgerAndCursor),
        // };
        // Ok(start)
    }
}