use crate::{
    commands::global,
    config::network,
    rpc,
    xdr::{self},
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Network(#[from] network::Error),
    #[error(transparent)]
    Rpc(#[from] rpc::Error),
    #[error(transparent)]
    Serde(#[from] serde_json::Error),
    #[error(transparent)]
    Xdr(#[from] xdr::Error),
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

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, clap::ValueEnum, Default)]
pub enum XdrFormat {
    /// XDR fields will be fetched as json and accessible via the headerJson and metadataJson fields
    #[default]
    Json,

    /// XDR fields will be fetched as xdr and accessible via the headerXdr and metadataXdr fields
    Xdr,
}

impl std::fmt::Display for XdrFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            XdrFormat::Json => write!(f, "json"),
            XdrFormat::Xdr => write!(f, "base64"),
        }
    }
}

#[derive(Debug, clap::Parser)]
pub struct Cmd {
    /// Ledger Sequence to start fetch (inclusive)
    pub seq: u32,

    /// Number of ledgers to fetch
    #[arg(long, default_value_t = 1)]
    pub limit: usize,

    #[command(flatten)]
    pub network: network::Args,

    /// Format of the output
    #[arg(long, value_enum, default_value_t)]
    pub output: OutputFormat,

    /// Format of the xdr in the output
    #[arg(long, value_enum, default_value_t)]
    pub xdr_format: XdrFormat,
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let network = self.network.get(&global_args.locator)?;
        let client = network.rpc_client()?;
        let start = rpc::LedgerStart::Ledger(self.seq);
        let result = client
            .get_ledgers(start, Some(self.limit), Some(self.xdr_format.to_string()))
            .await?;

        match self.output {
            OutputFormat::Text => {
                println!(
                    "Latest Ledger {} at close time {}",
                    result.latest_ledger, result.latest_ledger_close_time
                );
                println!(
                    "Oldest Ledger {} at close time {}",
                    result.oldest_ledger, result.oldest_ledger_close_time
                );
                println!("Ledgers (limit {} starting at {} )", self.limit, self.seq);
                println!("----------------------------------------------------\n");
                for ledger in result.ledgers.clone() {
                    println!("Ledger Sequence: {}", ledger.sequence);
                    println!("Hash: {}", ledger.hash);
                    println!("Close Time: {}", ledger.ledger_close_time);
                    match self.xdr_format {
                        XdrFormat::Json => {
                            println!(
                                "Header: {}",
                                serde_json::to_string_pretty(&ledger.header_json)?
                            );
                            println!(
                                "MetaData: {}",
                                serde_json::to_string_pretty(&ledger.metadata_json)?
                            );
                        }
                        XdrFormat::Xdr => {
                            println!(
                                "Header: {}",
                                serde_json::to_string_pretty(&ledger.header_xdr)?
                            );
                            println!(
                                "MetaData: {}",
                                serde_json::to_string_pretty(&ledger.metadata_xdr)?
                            );
                        }
                    }
                    println!("----------------------------------------------------\n");
                }
            }
            OutputFormat::Json => println!("{}", serde_json::to_string(&result)?),
            OutputFormat::JsonFormatted => println!("{}", serde_json::to_string_pretty(&result)?),
        }

        Ok(())
    }
}
