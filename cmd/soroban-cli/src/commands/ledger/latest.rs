use crate::{commands::global, config::network, rpc};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Serde(#[from] serde_json::Error),
    #[error(transparent)]
    Network(#[from] network::Error),
    #[error(transparent)]
    Rpc(#[from] rpc::Error),
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
    pub seq: Option<i64>,

    #[command(flatten)]
    pub network: network::Args,

    /// Format of the output
    #[arg(long, default_value = "text")]
    pub output: OutputFormat,
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let network = self.network.get(&global_args.locator)?;
        let client = network.rpc_client()?;
        let ledger = client.get_latest_ledger().await?;

        match self.output {
            OutputFormat::Text => {
                println!("Sequence: {}", ledger.sequence);
                println!("Protocol Version: {}", ledger.protocol_version);
                println!("ID: {}", ledger.id);
            }
            OutputFormat::Json => println!("{}", serde_json::to_string(&ledger)?),
            OutputFormat::JsonFormatted => println!("{}", serde_json::to_string_pretty(&ledger)?),
        }

        Ok(())
    }
}
