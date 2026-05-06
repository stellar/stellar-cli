use crate::{
    config::{
        locator,
        network::{self, Network},
    },
    rpc,
    xdr::LedgerKey,
};

#[derive(Debug, clap::Args, Clone)]
#[group(skip)]
pub struct Args {
    #[command(flatten)]
    pub network: network::Args,

    #[command(flatten)]
    pub locator: locator::Args,

    /// Format of the output
    #[arg(long, default_value = "json")]
    pub output: OutputFormat,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Network(#[from] network::Error),
    #[error(transparent)]
    Serde(#[from] serde_json::Error),
    #[error(transparent)]
    Rpc(#[from] rpc::Error),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, clap::ValueEnum, Default)]
pub enum OutputFormat {
    /// JSON output of the ledger entry with parsed XDRs (one line, not formatted)
    #[default]
    Json,
    /// Formatted (multiline) JSON output of the ledger entry with parsed XDRs
    JsonFormatted,
    /// Original RPC output (containing XDRs)
    Xdr,
}

impl Args {
    pub fn network(&self) -> Result<Network, Error> {
        Ok(self.network.get(&self.locator)?)
    }

    pub async fn run(&self, ledger_keys: Vec<LedgerKey>) -> Result<(), Error> {
        let network = self.network.get(&self.locator)?;
        let client = network.rpc_client()?;
        match self.output {
            OutputFormat::Json => {
                let resp = client.get_full_ledger_entries(&ledger_keys).await?;
                println!("{}", serde_json::to_string(&resp)?);
            }
            OutputFormat::Xdr => {
                let resp = client.get_ledger_entries(&ledger_keys).await?;
                println!("{}", serde_json::to_string(&resp)?);
            }
            OutputFormat::JsonFormatted => {
                let resp = client.get_full_ledger_entries(&ledger_keys).await?;
                println!("{}", serde_json::to_string_pretty(&resp)?);
            }
        }

        Ok(())
    }
}
