use soroban_rpc::GetTransactionResponse;

use crate::{
    commands::global,
    config::network,
    rpc,
    xdr::{self, Hash},
};

#[derive(Debug, Clone, clap::Parser)]
pub struct Args {
    /// Transaction hash to fetch
    #[arg(long)]
    pub hash: Hash,

    #[command(flatten)]
    pub network: network::Args,
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
    #[error("transaction {tx_hash} not found on {network} network")]
    NotFound { tx_hash: Hash, network: String },
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, clap::ValueEnum, Default)]
pub enum OutputFormat {
    /// JSON output with parsed XDRs (one line, not formatted)
    #[default]
    Json,
    /// Formatted (multiline) JSON output with parsed XDRs
    JsonFormatted,
    /// Original RPC output (containing XDRs)
    Xdr,
}

impl Args {
    pub async fn fetch_transaction(
        &self,
        global_args: &global::Args,
    ) -> Result<GetTransactionResponse, Error> {
        let network = self.network.get(&global_args.locator)?;
        let client = network.rpc_client()?;
        let tx_hash = self.hash.clone();
        let tx = client.get_transaction(&tx_hash).await?;
        match tx.status.clone() {
            val if val == *"NOT_FOUND" => {
                if let Some(n) = &self.network.network {
                    return Err(Error::NotFound {
                        tx_hash,
                        network: n.clone(),
                    });
                }
            }
            _ => {}
        }
        Ok(tx)
    }

    pub fn print_tx_summary(tx: &GetTransactionResponse) {
        println!("Transaction Status: {}", tx.status);
        if let Some(ledger) = tx.ledger {
            println!("Transaction Ledger: {ledger}");
        }
        println!();
    }
}
