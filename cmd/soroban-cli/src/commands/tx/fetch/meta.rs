use clap::{command, Parser};
use soroban_rpc::GetTransactionResponseRaw;
use crate::{
    xdr::{self, Hash},
    config::{
        locator,
        network::{self, Network},
    },
    rpc,
};

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    pub hash: Hash,

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
    pub async fn run(&self) -> Result<(), Error> {
        let network = self.network.get(&self.locator)?;
        let client = network.rpc_client()?;
        let tx_hash = self.hash.clone();
        match self.output {
            OutputFormat::Json => {
                let resp = client.get_transaction(&tx_hash).await?;
                let meta = resp.result_meta.unwrap();
                println!("{}", serde_json::to_string(&meta)?);
            }
            OutputFormat::Xdr => {
                let resp = client.get_transaction(&tx_hash).await?;
                let resp_as_xdr: GetTransactionResponseRaw = resp.clone().try_into()?;
                let meta = resp_as_xdr.result_meta_xdr;
                println!("{}", serde_json::to_string(&meta)?);
            }
            OutputFormat::JsonFormatted => {
                let resp = client.get_transaction(&tx_hash).await?;
                let meta = resp.result_meta.unwrap();
                println!("{}", serde_json::to_string_pretty(&meta)?);
            }
        }

        Ok(())
    }
}