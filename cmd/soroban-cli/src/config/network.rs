use clap::arg;
use phf::phf_map;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::str::FromStr;
use stellar_strkey::ed25519::PublicKey;
use url::Url;

use super::locator;
use crate::utils::http;
use crate::{
    commands::HEADING_RPC,
    rpc::{self, Client},
};
pub mod passphrase;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Config(#[from] locator::Error),
    #[error("network arg or rpc url and network passphrase are required if using the network")]
    Network,
    #[error(transparent)]
    Rpc(#[from] rpc::Error),
    #[error(transparent)]
    HttpClient(#[from] reqwest::Error),
    #[error("Failed to parse JSON from {0}, {1}")]
    FailedToParseJSON(String, serde_json::Error),
    #[error("Invalid URL {0}")]
    InvalidUrl(String),
    #[error("funding failed: {0}")]
    FundingFailed(String),
}

#[derive(Debug, clap::Args, Clone, Default)]
#[group(skip)]
pub struct Args {
    /// RPC server endpoint
    #[arg(
        long = "rpc-url",
        requires = "network_passphrase",
        required_unless_present = "network",
        env = "STELLAR_RPC_URL",
        help_heading = HEADING_RPC,
    )]
    pub rpc_url: Option<String>,
    /// Network passphrase to sign the transaction sent to the rpc server
    #[arg(
        long = "network-passphrase",
        requires = "rpc_url",
        required_unless_present = "network",
        env = "STELLAR_NETWORK_PASSPHRASE",
        help_heading = HEADING_RPC,
    )]
    pub network_passphrase: Option<String>,
    /// Name of network to use from config
    #[arg(
        long,
        required_unless_present = "rpc_url",
        required_unless_present = "network_passphrase",
        env = "STELLAR_NETWORK",
        help_heading = HEADING_RPC,
    )]
    pub network: Option<String>,
}

impl Args {
    pub fn get(&self, locator: &locator::Args) -> Result<Network, Error> {
        if let Some(name) = self.network.as_deref() {
            if let Ok(network) = locator.read_network(name) {
                return Ok(network);
            }
        }
        if let (Some(rpc_url), Some(network_passphrase)) =
            (self.rpc_url.clone(), self.network_passphrase.clone())
        {
            Ok(Network {
                rpc_url,
                network_passphrase,
            })
        } else {
            Err(Error::Network)
        }
    }
}

#[derive(Debug, clap::Args, Serialize, Deserialize, Clone)]
#[group(skip)]
pub struct Network {
    /// RPC server endpoint
    #[arg(
        long = "rpc-url",
        env = "STELLAR_RPC_URL",
        help_heading = HEADING_RPC,
    )]
    pub rpc_url: String,
    /// Network passphrase to sign the transaction sent to the rpc server
    #[arg(
            long,
            env = "STELLAR_NETWORK_PASSPHRASE",
            help_heading = HEADING_RPC,
        )]
    pub network_passphrase: String,
}

impl Network {
    pub async fn helper_url(&self, addr: &str) -> Result<Url, Error> {
        tracing::debug!("address {addr:?}");
        let rpc_url = Url::from_str(&self.rpc_url)
            .map_err(|_| Error::InvalidUrl(self.rpc_url.to_string()))?;
        if self.network_passphrase.as_str() == passphrase::LOCAL {
            let mut local_url = rpc_url;
            local_url.set_path("/friendbot");
            local_url.set_query(Some(&format!("addr={addr}")));
            Ok(local_url)
        } else {
            let client = Client::new(&self.rpc_url)?;
            let network = client.get_network().await?;
            tracing::debug!("network {network:?}");
            let url = client.friendbot_url().await?;
            tracing::debug!("URL {url:?}");
            Url::from_str(&format!("{url}?addr={addr}")).map_err(|e| {
                tracing::error!("{e}");
                Error::InvalidUrl(url.to_string())
            })
        }
    }

    #[allow(clippy::similar_names)]
    pub async fn fund_address(&self, addr: &PublicKey) -> Result<(), Error> {
        let uri = self.helper_url(&addr.to_string()).await?;
        tracing::debug!("URL {uri:?}");
        let response = http::client().get(uri.as_str()).send().await?;

        let request_successful = response.status().is_success();
        let body = response.bytes().await?;
        let res = serde_json::from_slice::<serde_json::Value>(&body)
            .map_err(|e| Error::FailedToParseJSON(uri.to_string(), e))?;
        tracing::debug!("{res:#?}");
        if !request_successful {
            if let Some(detail) = res.get("detail").and_then(Value::as_str) {
                if detail.contains("account already funded to starting balance") {
                    // Don't error if friendbot indicated that the account is
                    // already fully funded to the starting balance, because the
                    // user's goal is to get funded, and the account is funded
                    // so it is success much the same.
                    tracing::debug!("already funded error ignored because account is funded");
                } else {
                    return Err(Error::FundingFailed(detail.to_string()));
                }
            } else {
                return Err(Error::FundingFailed("unknown cause".to_string()));
            }
        }
        Ok(())
    }

    pub fn rpc_uri(&self) -> Result<Url, Error> {
        Url::from_str(&self.rpc_url).map_err(|_| Error::InvalidUrl(self.rpc_url.to_string()))
    }
}

pub static DEFAULTS: phf::Map<&'static str, (&'static str, &'static str)> = phf_map! {
    "local" => (
        "http://localhost:8000/rpc",
        passphrase::LOCAL,
    ),
    "futurenet" => (
        "https://rpc-futurenet.stellar.org:443",
        passphrase::FUTURENET,
    ),
    "testnet" => (
        "https://soroban-testnet.stellar.org",
        passphrase::TESTNET,
    ),
    "mainnet" => (
        "Bring Your Own: https://developers.stellar.org/docs/data/rpc/rpc-providers",
        passphrase::MAINNET,
    ),
};

impl From<&(&str, &str)> for Network {
    /// Convert the return value of `DEFAULTS.get()` into a Network
    fn from(n: &(&str, &str)) -> Self {
        Self {
            rpc_url: n.0.to_string(),
            network_passphrase: n.1.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_helper_url_local_network() {
        let network = Network {
            rpc_url: "http://localhost:8000".to_string(),
            network_passphrase: passphrase::LOCAL.to_string(),
        };

        let result = network
            .helper_url("GBZXN7PIRZGNMHGA7MUUUF4GWPY5AYPV6LY4UV2GL6VJGIQRXFDNMADI")
            .await;

        assert!(result.is_ok());
        let url = result.unwrap();
        assert_eq!(url.as_str(), "http://localhost:8000/friendbot?addr=GBZXN7PIRZGNMHGA7MUUUF4GWPY5AYPV6LY4UV2GL6VJGIQRXFDNMADI");
    }

    #[tokio::test]
    async fn test_helper_url_test_network() {
        // It might be a bit difficult to conduct client mock here.
        let friendbot_url = "https://friendbot.stellar.org/secret_key";
        let addr = "GBZXN7PIRZGNMHGA7MUUUF4GWPY5AYPV6LY4UV2GL6VJGIQRXFDNMADI";
        let url = Url::from_str(&format!("{friendbot_url}?addr={addr}")).unwrap();
        assert_eq!(url.as_str(), "https://friendbot.stellar.org/secret_key?addr=GBZXN7PIRZGNMHGA7MUUUF4GWPY5AYPV6LY4UV2GL6VJGIQRXFDNMADI");
    }
}
