use itertools::Itertools;
use phf::phf_map;
use reqwest::header::HeaderMap;
use reqwest::header::{HeaderName, HeaderValue, InvalidHeaderName, InvalidHeaderValue};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::str::FromStr;
use stellar_strkey::ed25519::PublicKey;
use url::Url;

use super::locator;
use crate::utils::{http, url::redact_url};
use crate::{
    commands::HEADING_RPC,
    rpc::{self, Client},
};
pub mod passphrase;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Config(#[from] locator::Error),
    #[error(
        r#"Access to the network is required
`--network` or `--rpc-url` and `--network-passphrase` are required if using the network.
Network configuration can also be set using `network use` subcommand. For example, to use
testnet, run `stellar network use testnet`.
Alternatively you can use their corresponding environment variables:
STELLAR_NETWORK, STELLAR_RPC_URL and STELLAR_NETWORK_PASSPHRASE"#
    )]
    Network,
    #[error(
        "rpc-url is used but network passphrase is missing, use `--network-passphrase` or `STELLAR_NETWORK_PASSPHRASE`"
    )]
    MissingNetworkPassphrase,
    #[error(
        "network passphrase is used but rpc-url is missing, use `--rpc-url` or `STELLAR_RPC_URL`"
    )]
    MissingRpcUrl,
    #[error("cannot use both `--rpc-url` and `--network`")]
    CannotUseBothRpcAndNetwork,
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
    #[error(transparent)]
    InvalidHeaderName(#[from] InvalidHeaderName),
    #[error(transparent)]
    InvalidHeaderValue(#[from] InvalidHeaderValue),
    #[error("invalid HTTP header: must be in the form 'key:value'")]
    InvalidHeader,
}

#[derive(Debug, clap::Args, Clone, Default)]
#[group(id = "network-args")]
pub struct Args {
    /// RPC server endpoint
    #[arg(
        long = "rpc-url",
        env = "STELLAR_RPC_URL",
        help_heading = HEADING_RPC,
    )]
    pub rpc_url: Option<String>,
    /// RPC Header(s) to include in requests to the RPC provider, example: "X-API-Key: abc123". Multiple headers can be added by passing the option multiple times.
    #[arg(
        long = "rpc-header",
        env = "STELLAR_RPC_HEADERS",
        help_heading = HEADING_RPC,
        num_args = 1,
        action = clap::ArgAction::Append,
        value_delimiter = '\n',
        hide_env_values = true,
    )]
    pub rpc_headers: Vec<String>,
    /// Network passphrase to sign the transaction sent to the rpc server
    #[arg(
        long = "network-passphrase",
        env = "STELLAR_NETWORK_PASSPHRASE",
        help_heading = HEADING_RPC,
    )]
    pub network_passphrase: Option<String>,
    /// Name of network to use from config
    #[arg(
        long,
        short = 'n',
        env = "STELLAR_NETWORK",
        help_heading = HEADING_RPC,
    )]
    pub network: Option<String>,
}

impl Args {
    pub fn get(&self, locator: &locator::Args) -> Result<Network, Error> {
        match (
            self.network.as_deref(),
            self.rpc_url.clone(),
            self.network_passphrase.clone(),
        ) {
            (None, None, None) => {
                // Fall back to testnet as the default network if no config default is set
                Ok(DEFAULTS.get(DEFAULT_NETWORK_KEY).unwrap().into())
            }
            (_, Some(_), None) => Err(Error::MissingNetworkPassphrase),
            (_, None, Some(_)) => Err(Error::MissingRpcUrl),
            (Some(network), None, None) => Ok(locator.read_network(network)?),
            (_, Some(rpc_url), Some(network_passphrase)) => {
                let rpc_headers = self
                    .rpc_headers
                    .iter()
                    .map(|h| parse_http_header(h))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(Network {
                    rpc_url,
                    rpc_headers,
                    network_passphrase,
                })
            }
        }
    }
}

#[derive(clap::Args, Serialize, Deserialize, Clone)]
#[group(skip)]
pub struct Network {
    /// RPC server endpoint
    #[arg(
        long = "rpc-url",
        env = "STELLAR_RPC_URL",
        help_heading = HEADING_RPC,
    )]
    pub rpc_url: String,
    /// Optional header to include in requests to the RPC, example: "X-API-Key: abc123". Multiple headers can be added by passing the option multiple times.
    #[arg(
        long = "rpc-header",
        env = "STELLAR_RPC_HEADERS",
        help_heading = HEADING_RPC,
        num_args = 1,
        action = clap::ArgAction::Append,
        value_delimiter = '\n',
        value_parser = accept_raw_rpc_header,
        hide_env_values = true,
    )]
    pub rpc_headers: Vec<(String, String)>,
    /// Network passphrase to sign the transaction sent to the rpc server
    #[arg(
            long,
            env = "STELLAR_NETWORK_PASSPHRASE",
            help_heading = HEADING_RPC,
        )]
    pub network_passphrase: String,
}

impl std::fmt::Debug for Network {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let concealed: Vec<(&str, &str)> = self
            .rpc_headers
            .iter()
            .map(|(k, _)| (k.as_str(), "<concealed>"))
            .collect();
        f.debug_struct("Network")
            .field("rpc_url", &redact_url(&self.rpc_url))
            .field("rpc_headers", &concealed)
            .field("network_passphrase", &self.network_passphrase)
            .finish()
    }
}

fn parse_http_header(header: &str) -> Result<(String, String), Error> {
    let header_components = header.splitn(2, ':');

    let (key, value) = header_components
        .map(str::trim)
        .next_tuple()
        .ok_or_else(|| Error::InvalidHeader)?;

    HeaderName::from_str(key)?;
    HeaderValue::from_str(value)?;

    Ok((key.to_string(), value.to_string()))
}

/// Clap value_parser for `Network::rpc_headers` that always succeeds, deferring
/// validation to application code so clap never echoes the raw value in error messages.
#[allow(clippy::unnecessary_wraps)]
fn accept_raw_rpc_header(header: &str) -> Result<(String, String), std::convert::Infallible> {
    match header.split_once(':') {
        Some((key, value)) => Ok((key.trim().to_string(), value.trim().to_string())),
        None => Ok((String::new(), header.to_string())),
    }
}

fn validate_rpc_headers(headers: &[(String, String)]) -> Result<(), Error> {
    for (key, value) in headers {
        HeaderName::from_str(key).map_err(|_| Error::InvalidHeader)?;
        HeaderValue::from_str(value).map_err(|_| Error::InvalidHeader)?;
    }
    Ok(())
}

impl Network {
    pub fn validate_headers(&self) -> Result<(), Error> {
        validate_rpc_headers(&self.rpc_headers)
    }

    pub async fn helper_url(&self, addr: &str) -> Result<Url, Error> {
        tracing::debug!("address {addr:?}");
        let rpc_url = Url::from_str(&self.rpc_url)
            .map_err(|_| Error::InvalidUrl(redact_url(&self.rpc_url)))?;
        if self.network_passphrase.as_str() == passphrase::LOCAL {
            let mut local_url = rpc_url;
            local_url.set_path("/friendbot");
            local_url.set_query(Some(&format!("addr={addr}")));
            Ok(local_url)
        } else {
            let client = self.rpc_client()?;
            let network = client.get_network().await?;
            tracing::debug!(
                "network passphrase={:?} protocol_version={} friendbot_url={:?}",
                network.passphrase,
                network.protocol_version,
                network.friendbot_url.as_deref().map(redact_url),
            );
            let url = client.friendbot_url().await?;
            tracing::debug!("URL {}", redact_url(&url));
            let mut url = Url::from_str(&url).map_err(|e| {
                tracing::error!("{e}");
                Error::InvalidUrl(redact_url(&url))
            })?;
            url.query_pairs_mut().append_pair("addr", addr);
            Ok(url)
        }
    }

    #[allow(clippy::similar_names)]
    pub async fn fund_address(&self, addr: &PublicKey) -> Result<(), Error> {
        let uri = self.helper_url(&addr.to_string()).await?;
        tracing::debug!("URL {}", redact_url(uri.as_str()));
        let response = http::client().get(uri.as_str()).send().await?;

        let request_successful = response.status().is_success();
        let body = response.bytes().await?;
        let res = serde_json::from_slice::<serde_json::Value>(&body)
            .map_err(|e| Error::FailedToParseJSON(redact_url(uri.as_str()), e))?;
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
        Url::from_str(&self.rpc_url).map_err(|_| Error::InvalidUrl(redact_url(&self.rpc_url)))
    }

    pub fn rpc_client(&self) -> Result<Client, Error> {
        let mut header_hash_map = HashMap::new();
        for (header_name, header_value) in &self.rpc_headers {
            header_hash_map.insert(header_name.clone(), header_value.clone());
        }

        let header_map: HeaderMap = (&header_hash_map)
            .try_into()
            .map_err(|_| Error::InvalidHeader)?;

        rpc::Client::new_with_headers(&self.rpc_url, header_map).map_err(|e| match e {
            rpc::Error::InvalidRpcUrl(..) | rpc::Error::InvalidRpcUrlFromUriParts(..) => {
                Error::InvalidUrl(redact_url(&self.rpc_url))
            }
            other => Error::Rpc(other),
        })
    }
}

/// Default network key to use when no network is specified
pub const DEFAULT_NETWORK_KEY: &str = "testnet";

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
            rpc_headers: Vec::new(),
            network_passphrase: n.1.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::Server;
    use serde_json::json;

    const INVALID_HEADER_NAME: &str = "api key";
    const INVALID_HEADER_VALUE: &str = "cannot include a carriage return \r in the value";

    #[tokio::test]
    async fn test_helper_url_local_network() {
        let network = Network {
            rpc_url: "http://localhost:8000".to_string(),
            network_passphrase: passphrase::LOCAL.to_string(),
            rpc_headers: Vec::new(),
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
        let mut server = Server::new_async().await;
        let _mock = server
            .mock("POST", "/")
            .with_body_from_request(|req| {
                let body: Value = serde_json::from_slice(req.body().unwrap()).unwrap();
                let id = body["id"].clone();
                json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "friendbotUrl": "https://friendbot.stellar.org/",
                            "passphrase": passphrase::TESTNET.to_string(),
                            "protocolVersion": 21
                    }
                })
                .to_string()
                .into()
            })
            .create_async()
            .await;

        let network = Network {
            rpc_url: server.url(),
            network_passphrase: passphrase::TESTNET.to_string(),
            rpc_headers: Vec::new(),
        };
        let url = network
            .helper_url("GBZXN7PIRZGNMHGA7MUUUF4GWPY5AYPV6LY4UV2GL6VJGIQRXFDNMADI")
            .await
            .unwrap();
        assert_eq!(url.as_str(), "https://friendbot.stellar.org/?addr=GBZXN7PIRZGNMHGA7MUUUF4GWPY5AYPV6LY4UV2GL6VJGIQRXFDNMADI");
    }

    #[tokio::test]
    async fn test_helper_url_test_network_with_path_and_params() {
        let mut server = Server::new_async().await;
        let _mock = server.mock("POST", "/")
            .with_body_from_request(|req| {
                let body: Value = serde_json::from_slice(req.body().unwrap()).unwrap();
                let id = body["id"].clone();
                json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "friendbotUrl": "https://friendbot.stellar.org/secret?api_key=123456&user=demo",
                            "passphrase": passphrase::TESTNET.to_string(),
                            "protocolVersion": 21
                    }
                }).to_string().into()
            })
            .create_async().await;

        let network = Network {
            rpc_url: server.url(),
            network_passphrase: passphrase::TESTNET.to_string(),
            rpc_headers: Vec::new(),
        };
        let url = network
            .helper_url("GBZXN7PIRZGNMHGA7MUUUF4GWPY5AYPV6LY4UV2GL6VJGIQRXFDNMADI")
            .await
            .unwrap();
        assert_eq!(url.as_str(), "https://friendbot.stellar.org/secret?api_key=123456&user=demo&addr=GBZXN7PIRZGNMHGA7MUUUF4GWPY5AYPV6LY4UV2GL6VJGIQRXFDNMADI");
    }

    // testing parse_header function
    #[tokio::test]
    async fn test_parse_http_header_ok() {
        let result = parse_http_header("Authorization: Bearer 1234");
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_parse_http_header_error_with_invalid_name() {
        let invalid_header = format!("{INVALID_HEADER_NAME}: Bearer 1234");
        let result = parse_http_header(&invalid_header);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            format!("invalid HTTP header name")
        );
    }

    #[tokio::test]
    async fn test_parse_http_header_error_with_invalid_value() {
        let invalid_header = format!("Authorization: {INVALID_HEADER_VALUE}");
        let result = parse_http_header(&invalid_header);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            format!("failed to parse header value")
        );
    }

    // testing rpc_client function - we're testing this and the parse_http_header function separately because when a user has their network already configured in a toml file, the parse_http_header function is not called and we want to make sure that if the toml file is correctly formatted, the rpc_client function will work as expected

    #[tokio::test]
    async fn test_rpc_client_is_ok_when_there_are_no_headers() {
        let network = Network {
            rpc_url: "http://localhost:1234".to_string(),
            network_passphrase: "Network passphrase".to_string(),
            rpc_headers: [].to_vec(),
        };

        let result = network.rpc_client();
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_rpc_client_is_ok_with_correctly_formatted_headers() {
        let network = Network {
            rpc_url: "http://localhost:1234".to_string(),
            network_passphrase: "Network passphrase".to_string(),
            rpc_headers: [("Authorization".to_string(), "Bearer 1234".to_string())].to_vec(),
        };

        let result = network.rpc_client();
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_rpc_client_is_ok_with_multiple_headers() {
        let network = Network {
            rpc_url: "http://localhost:1234".to_string(),
            network_passphrase: "Network passphrase".to_string(),
            rpc_headers: [
                ("Authorization".to_string(), "Bearer 1234".to_string()),
                ("api-key".to_string(), "5678".to_string()),
            ]
            .to_vec(),
        };

        let result = network.rpc_client();
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_rpc_client_returns_err_with_invalid_header_name() {
        let network = Network {
            rpc_url: "http://localhost:8000".to_string(),
            network_passphrase: passphrase::LOCAL.to_string(),
            rpc_headers: [(INVALID_HEADER_NAME.to_string(), "Bearer".to_string())].to_vec(),
        };

        let result = network.rpc_client();
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            format!("invalid HTTP header: must be in the form 'key:value'")
        );
    }

    #[tokio::test]
    async fn test_rpc_client_returns_err_with_invalid_header_value() {
        let network = Network {
            rpc_url: "http://localhost:8000".to_string(),
            network_passphrase: passphrase::LOCAL.to_string(),
            rpc_headers: [("api-key".to_string(), INVALID_HEADER_VALUE.to_string())].to_vec(),
        };

        let result = network.rpc_client();
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            format!("invalid HTTP header: must be in the form 'key:value'")
        );
    }

    #[tokio::test]
    async fn test_rpc_client_returns_err_with_bad_rpc_url() {
        let network = Network {
            rpc_url: "Bring Your Own: http://localhost:8000".to_string(),
            network_passphrase: passphrase::LOCAL.to_string(),
            rpc_headers: [].to_vec(),
        };

        let result = network.rpc_client();
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            format!("Invalid URL Bring Your Own: http://localhost:8000")
        );
    }

    #[tokio::test]
    async fn test_default_to_testnet_when_no_network_specified() {
        use super::super::locator;

        let args = Args::default(); // No network, rpc_url, or network_passphrase specified
        let locator_args = locator::Args::default();

        let result = args.get(&locator_args);
        assert!(result.is_ok());

        let network = result.unwrap();
        assert_eq!(network.network_passphrase, passphrase::TESTNET);
        assert_eq!(network.rpc_url, "https://soroban-testnet.stellar.org");
    }

    #[tokio::test]
    async fn test_user_config_default_overrides_automatic_testnet() {
        use super::super::locator;
        use std::env;

        // Override environment variables to prevent reading real user config
        let original_home = env::var("HOME").ok();
        let original_stellar_config_home = env::var("STELLAR_CONFIG_HOME").ok();

        // Set to a non-existent directory to ensure Config::new() fails and we test the fallback
        env::set_var("HOME", "/dev/null");
        env::set_var("STELLAR_CONFIG_HOME", "/dev/null");

        let args = Args::default(); // No network, rpc_url, or network_passphrase specified
        let locator_args = locator::Args::default();

        let result = args.get(&locator_args);
        assert!(result.is_ok());

        let network = result.unwrap();
        // Should still default to testnet when config reading fails
        assert_eq!(network.network_passphrase, passphrase::TESTNET);
        assert_eq!(network.rpc_url, "https://soroban-testnet.stellar.org");

        // Restore original environment variables
        if let Some(home) = original_home {
            env::set_var("HOME", home);
        } else {
            env::remove_var("HOME");
        }
        if let Some(config_home) = original_stellar_config_home {
            env::set_var("STELLAR_CONFIG_HOME", config_home);
        } else {
            env::remove_var("STELLAR_CONFIG_HOME");
        }
    }

    #[test]
    fn test_malformed_rpc_header_accepted_by_clap_without_error() {
        use crate::test_utils::with_env_guard;
        use clap::Parser;

        #[derive(clap::Parser)]
        struct TestCmd {
            #[command(flatten)]
            args: Args,
        }

        let secret = "Authorization Bearer secret_poc_token_12345";
        with_env_guard(&["STELLAR_RPC_HEADERS"], || {
            std::env::set_var("STELLAR_RPC_HEADERS", secret);
            let result = TestCmd::try_parse_from(["stellar"]);
            assert!(
                result.is_ok(),
                "Clap must accept malformed RPC headers without error — validation is deferred to application code to prevent secrets from being echoed in clap error messages"
            );
        });
    }

    #[test]
    fn test_validate_headers_rejects_missing_colon_without_exposing_value() {
        // Simulates what accept_raw_rpc_header stores when no ':' is present.
        let network = Network {
            rpc_url: "http://localhost:8000".to_string(),
            network_passphrase: "Test".to_string(),
            rpc_headers: vec![(
                String::new(),
                "Authorization Bearer secret_token_xyz".to_string(),
            )],
        };

        let result = network.validate_headers();
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert_eq!(
            error_msg,
            "invalid HTTP header: must be in the form 'key:value'"
        );
        assert!(
            !error_msg.contains("secret_token_xyz"),
            "Error must not expose the raw header value, got: {error_msg}"
        );
    }

    #[test]
    fn test_malformed_rpc_header_app_error_does_not_expose_value() {
        use super::super::locator;

        let secret = "Authorization Bearer secret_poc_token_12345";
        let args = Args {
            rpc_url: Some("https://example.com".to_string()),
            rpc_headers: vec![secret.to_string()],
            network_passphrase: Some("Test SDF Network ; September 2015".to_string()),
            network: None,
        };

        let result = args.get(&locator::Args::default());
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(
            !error_msg.contains("secret_poc_token_12345"),
            "Application error must not expose secret header value, got: {error_msg}"
        );
    }

    #[test]
    fn test_debug_conceals_rpc_header_values() {
        let network = Network {
            rpc_url: "http://localhost:8000/rpc".to_string(),
            network_passphrase: "Test Network".to_string(),
            rpc_headers: vec![
                ("Authorization".to_string(), "Bearer secret123".to_string()),
                ("X-Api-Key".to_string(), "mykey".to_string()),
            ],
        };
        assert_eq!(
            format!("{network:?}"),
            r#"Network { rpc_url: "http://localhost:8000/rpc", rpc_headers: [("Authorization", "<concealed>"), ("X-Api-Key", "<concealed>")], network_passphrase: "Test Network" }"#
        );
    }

    #[test]
    fn test_debug_conceals_rpc_url_password() {
        let network = Network {
            rpc_url: "https://alice:supersecret@rpc.example.com/soroban".to_string(),
            network_passphrase: "Test Network".to_string(),
            rpc_headers: Vec::new(),
        };
        let rendered = format!("{network:?}");
        assert!(
            !rendered.contains("supersecret"),
            "password leaked into Debug output: {rendered}"
        );
        assert!(
            rendered.contains("alice:redacted"),
            "expected `alice:redacted` in Debug output: {rendered}"
        );
    }

    #[tokio::test]
    async fn fund_address_failed_to_parse_json_does_not_leak_credentialed_rpc_url() {
        let mut server = Server::new_async().await;
        // Friendbot returns a non-JSON body so serde_json::from_slice fails,
        // triggering Error::FailedToParseJSON at the line we want to verify.
        let _mock = server
            .mock("GET", mockito::Matcher::Any)
            .with_status(200)
            .with_body("not valid json")
            .create_async()
            .await;

        let host_port = server
            .url()
            .strip_prefix("http://")
            .expect("mockito url starts with http://")
            .to_string();
        let credentialed_rpc_url = format!("http://alice:supersecret@{host_port}");

        let network = Network {
            rpc_url: credentialed_rpc_url,
            network_passphrase: passphrase::LOCAL.to_string(),
            rpc_headers: Vec::new(),
        };

        let addr =
            PublicKey::from_string("GBZXN7PIRZGNMHGA7MUUUF4GWPY5AYPV6LY4UV2GL6VJGIQRXFDNMADI")
                .unwrap();
        let err = network
            .fund_address(&addr)
            .await
            .expect_err("fund_address must return Err when friendbot replies with non-JSON body");
        let rendered = err.to_string();
        assert!(
            !rendered.contains("supersecret"),
            "password leaked into error display: {rendered}"
        );
        assert!(
            rendered.contains("alice:redacted"),
            "expected `alice:redacted` placeholder in error display: {rendered}"
        );
    }

    #[tokio::test]
    async fn helper_url_returned_credentialed_url_is_redactable_at_display_sinks() {
        // Non-LOCAL passphrase branch: helper_url asks the RPC for the friendbot URL.
        // The mocked RPC returns a parseable URL carrying userinfo, so Url::from_str
        // succeeds and helper_url returns Ok(url). The InvalidUrl branch is therefore
        // not exercised here — driving it would require an unparseable URL, which by
        // design leaks unchanged (see PR discussion). This test only documents that
        // the parseable URL returned from helper_url can be safely run through
        // redact_url at any subsequent display sink.
        let mut server = Server::new_async().await;
        let _mock = server
            .mock("POST", "/")
            .with_body_from_request(|req| {
                let body: Value = serde_json::from_slice(req.body().unwrap()).unwrap();
                let id = body["id"].clone();
                // Returned friendbot URL has userinfo + is parseable by url::Url.
                // Url::from_str inside helper_url accepts it, so the InvalidUrl
                // path at line 239 isn't exercised. Instead the URL flows into
                // the tracing line and (after fund_address) into FailedToParseJSON.
                json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "friendbotUrl": "https://alice:supersecret@friendbot.example/",
                        "passphrase": passphrase::TESTNET.to_string(),
                        "protocolVersion": 21,
                    }
                })
                .to_string()
                .into()
            })
            .create_async()
            .await;

        let network = Network {
            rpc_url: server.url(),
            network_passphrase: passphrase::TESTNET.to_string(),
            rpc_headers: Vec::new(),
        };
        let returned = network
            .helper_url("GBZXN7PIRZGNMHGA7MUUUF4GWPY5AYPV6LY4UV2GL6VJGIQRXFDNMADI")
            .await
            .expect("helper_url should accept a parseable credentialed friendbot URL");
        // The Url returned still carries the password — callers need it to authenticate.
        assert_eq!(returned.password(), Some("supersecret"));
        let redacted_for_display = redact_url(returned.as_str());
        assert!(
            !redacted_for_display.contains("supersecret"),
            "redact_url failed to redact a parseable friendbot URL: {redacted_for_display}"
        );
    }
}
