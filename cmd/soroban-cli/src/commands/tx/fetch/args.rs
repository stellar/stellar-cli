use crate::color::{green, red};
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
    #[error("transaction {tx_hash} not found")]
    NotFoundGeneric { tx_hash: Hash },
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
        if tx.status == "NOT_FOUND" {
            let network_name = self.network.network.clone().or_else(|| {
                match network.network_passphrase.as_str() {
                    network::passphrase::TESTNET => Some("testnet".to_string()),
                    network::passphrase::MAINNET => Some("mainnet".to_string()),
                    network::passphrase::FUTURENET => Some("futurenet".to_string()),
                    network::passphrase::LOCAL => Some("local".to_string()),
                    _ => None,
                }
            });

            return if let Some(n) = network_name {
                Err(Error::NotFound {
                    tx_hash,
                    network: n,
                })
            } else {
                Err(Error::NotFoundGeneric { tx_hash })
            };
        }
        Ok(tx)
    }

    pub fn print_tx_summary(tx: &GetTransactionResponse) {
        let status = if tx.status == "SUCCESS" {
            green(&tx.status)
        } else {
            red(&tx.status)
        };
        println!("Transaction Status: {status}");
        if let Some(ledger) = tx.ledger {
            println!("Transaction Ledger: {ledger}");
        }
        println!();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_not_found_formatting() {
        let hash = Hash([0u8; 32]);
        let err = Error::NotFound {
            tx_hash: hash.clone(),
            network: "testnet".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "transaction 0000000000000000000000000000000000000000000000000000000000000000 not found on testnet network"
        );

        let err_generic = Error::NotFoundGeneric { tx_hash: hash };
        assert_eq!(
            err_generic.to_string(),
            "transaction 0000000000000000000000000000000000000000000000000000000000000000 not found"
        );
    }
}
