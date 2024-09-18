use std::str::FromStr;

use clap::arg;
use phf::phf_map;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use stellar_strkey::ed25519::PublicKey;

use crate::{
    commands::HEADING_RPC,
    rpc::{self, Client},
};

use super::locator;
pub mod passphrase;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Config(#[from] locator::Error),

    #[error("please provide a network; use --network or set SOROBAN_NETWORK env var")]
    Network,
    #[error(transparent)]
    Http(#[from] http::Error),
    #[error(transparent)]
    Rpc(#[from] rpc::Error),
    #[error(transparent)]
    Hyper(#[from] hyper::Error),
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
    /// Name of network to use from config
    #[arg(
        long,
        env = "STELLAR_NETWORK",
        help_heading = HEADING_RPC,
        global = true,
    )]
    pub network: Option<String>,
}

impl Args {
    pub fn get(&self, locator: &locator::Args) -> Result<Network, Error> {
        if let Some(network) = &self.network {
            if let Ok(network) = locator.read_network(network.as_str()) {
                return Ok(network);
            }
        }

        Err(Error::Network)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Network {
    /// RPC server endpoint
    pub rpc_url: String,

    /// Network passphrase to sign the transaction sent to the rpc server
    pub network_passphrase: String,

    /// The alias name for the network entry
    pub name: String,
}

impl Network {
    pub async fn helper_url(&self, addr: &str) -> Result<http::Uri, Error> {
        use http::Uri;
        tracing::debug!("address {addr:?}");
        let rpc_uri = Uri::from_str(&self.rpc_url)
            .map_err(|_| Error::InvalidUrl(self.rpc_url.to_string()))?;
        if self.network_passphrase.as_str() == passphrase::LOCAL {
            let auth = rpc_uri.authority().unwrap().clone();
            let scheme = rpc_uri.scheme_str().unwrap();
            Ok(Uri::builder()
                .authority(auth)
                .scheme(scheme)
                .path_and_query(format!("/friendbot?addr={addr}"))
                .build()?)
        } else {
            let client = Client::new(&self.rpc_url)?;
            let network = client.get_network().await?;
            tracing::debug!("network {network:?}");
            let uri = client.friendbot_url().await?;
            tracing::debug!("URI {uri:?}");
            Uri::from_str(&format!("{uri}?addr={addr}")).map_err(|e| {
                tracing::error!("{e}");
                Error::InvalidUrl(uri.to_string())
            })
        }
    }

    #[allow(clippy::similar_names)]
    pub async fn fund_address(&self, addr: &PublicKey) -> Result<(), Error> {
        let uri = self.helper_url(&addr.to_string()).await?;
        tracing::debug!("URL {uri:?}");
        let response = match uri.scheme_str() {
            Some("http") => hyper::Client::new().get(uri.clone()).await?,
            Some("https") => {
                let https = hyper_tls::HttpsConnector::new();
                hyper::Client::builder()
                    .build::<_, hyper::Body>(https)
                    .get(uri.clone())
                    .await?
            }
            _ => {
                return Err(Error::InvalidUrl(uri.to_string()));
            }
        };
        let request_successful = response.status().is_success();
        let body = hyper::body::to_bytes(response.into_body()).await?;
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

    pub fn rpc_uri(&self) -> Result<http::Uri, Error> {
        http::Uri::from_str(&self.rpc_url).map_err(|_| Error::InvalidUrl(self.rpc_url.to_string()))
    }
}

pub static DEFAULTS: phf::Map<&'static str, (&'static str, &'static str, &'static str)> = phf_map! {
    "local" => (
        "local",
        "http://localhost:8000/rpc",
        passphrase::LOCAL,
    ),
    "futurenet" => (
        "futurenet",
        "https://rpc-futurenet.stellar.org:443",
        passphrase::FUTURENET,
    ),
    "testnet" => (
        "testnet",
        "https://soroban-testnet.stellar.org",
        passphrase::TESTNET,
    ),
    "mainnet" => (
        "mainnet",
        "Bring Your Own: https://developers.stellar.org/docs/data/rpc/rpc-providers",
        passphrase::MAINNET,
    ),
};

impl From<&(&str, &str, &str)> for Network {
    /// Convert the return value of `DEFAULTS.get()` into a Network
    fn from(n: &(&str, &str, &str)) -> Self {
        Self {
            name: n.0.to_string(),
            rpc_url: n.1.to_string(),
            network_passphrase: n.2.to_string(),
        }
    }
}
