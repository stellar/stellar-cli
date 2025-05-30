use crate::{
    config::{
        locator,
        network::{self, Network},
    },
    rpc,
    xdr::{self, Hash},
};

#[derive(Debug, Clone, clap::Parser)]
// #[group(skip)]
pub struct Args {
    #[arg(long)]
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

impl Args {
    // pub async fn run(&self) -> Result<(), Error> {
    //         let network = self.network.get(&self.locator)?;
    //         let client = network.rpc_client()?;
    //         let tx_hash = self.hash.clone();
    //         match self.output {
    //             OutputFormat::Json => {
    //                 let resp = client.get_transaction(&tx_hash).await?;
    //                 let envelope = resp.envelope.unwrap();
    //                 println!("{}", serde_json::to_string(&envelope)?);
    //             }
    //             OutputFormat::Xdr => {
    //                 let resp = client.get_transaction(&tx_hash).await?;
    //                 let resp_as_xdr: GetTransactionResponseRaw = resp.clone().try_into()?;
    //                 let envelope = resp_as_xdr.envelope_xdr;
    //                 println!("{}", serde_json::to_string(&envelope)?);
    //             }
    //             OutputFormat::JsonFormatted => {
    //                 let resp = client.get_transaction(&tx_hash).await?;
    //                 let envelope = resp.envelope.unwrap();
    //                 println!("{}", serde_json::to_string_pretty(&envelope)?);
    //             }
    //         }
    //         Ok(())
    //     }
}
