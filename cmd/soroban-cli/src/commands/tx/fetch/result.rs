use crate::{
    commands::global,
    config::network,
    rpc,
    xdr::{self, Hash, Limits, WriteXdr},
};
use clap::{command, Parser};

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    /// Transaction hash to fetch
    pub hash: Hash,

    #[command(flatten)]
    pub network: network::Args,

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
    #[error(transparent)]
    Xdr(#[from] xdr::Error),
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

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let network = self.network.get(&global_args.locator)?;
        let client = network.rpc_client()?;
        let tx_hash = self.hash.clone();
        match self.output {
            OutputFormat::Json => {
                let resp = client.get_transaction(&tx_hash).await?;
                if let Some(result) = resp.result {
                    println!("{}", serde_json::to_string(&result)?);
                }
            }
            OutputFormat::Xdr => {
                let resp = client.get_transaction(&tx_hash).await?;
                if let Some(result) = resp.result {
                    let result_xdr = result.to_xdr_base64(Limits::none()).unwrap();
                    println!("{}", &result_xdr);
                }
            }
            OutputFormat::JsonFormatted => {
                let resp = client.get_transaction(&tx_hash).await?;
                if let Some(result) = resp.result {
                    println!("{}", serde_json::to_string_pretty(&result)?);
                }
            }
        }

        Ok(())
    }
}
