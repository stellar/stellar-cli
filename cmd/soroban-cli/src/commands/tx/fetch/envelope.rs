use crate::{
    commands::global,
    config::network,
    rpc,
    xdr::{self, Hash, Limits, WriteXdr},
};
use clap::{command, Parser};

use super::args;

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    /// Transaction hash to fetch
    #[arg(long)]
    pub hash: Hash,

    #[command(flatten)]
    pub network: network::Args,

    /// Format of the output
    #[arg(long, default_value = "json")]
    pub output: args::OutputFormat,
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

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let network = self.network.get(&global_args.locator)?;
        let client = network.rpc_client()?;
        let tx_hash = self.hash.clone();
        match self.output {
            args::OutputFormat::Json => {
                let resp = client.get_transaction(&tx_hash).await?;
                if let Some(envelope) = resp.envelope {
                    println!("{}", serde_json::to_string(&envelope)?);
                }
            }
            args::OutputFormat::Xdr => {
                let resp = client.get_transaction(&tx_hash).await?;
                if let Some(envelope) = resp.envelope {
                    let envelope_xdr = envelope.to_xdr_base64(Limits::none()).unwrap();
                    println!("{envelope_xdr}");
                }
            }
            args::OutputFormat::JsonFormatted => {
                let resp = client.get_transaction(&tx_hash).await?;
                if let Some(envelope) = resp.envelope {
                    println!("{}", serde_json::to_string_pretty(&envelope)?);
                }
            }
        }

        Ok(())
    }
}
