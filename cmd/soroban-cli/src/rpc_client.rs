use std::ops::{Deref, DerefMut};

use http::{HeaderMap, HeaderName, HeaderValue};

use crate::config::network::Network;
use crate::rpc;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Rpc(#[from] rpc::Error),
    #[error("Invalid header: {0}")]
    InvalidHeader(String),
    #[error(transparent)]
    InvalidHeaderName(#[from] http::header::InvalidHeaderName),
    #[error(transparent)]
    InvalidHeaderValue(#[from] http::header::InvalidHeaderValue),
}

// todo:
// allow the user to pass in multiple headers

#[derive(Debug)]
pub struct RpcClient {
    client: rpc::Client,
}

impl RpcClient {
    pub fn new(network: Network) -> Result<Self, Error> {
        let mut additional_headers = HeaderMap::new();
        if let Some(rpc_header) = network.rpc_header {
            let header_components = rpc_header.split(':').collect::<Vec<&str>>();
            if header_components.len() != 2 {
                return Err(Error::InvalidHeader(format!(
                    "Missing a header name and/or value: {rpc_header}"
                )));
            }
            let key = header_components[0];
            let value = header_components[1];

            let header_name = HeaderName::from_bytes(key.as_bytes())?;
            let header_value = HeaderValue::from_str(value)?;

            additional_headers.insert(header_name, header_value);
        }

        let client = rpc::Client::new_with_headers(&network.rpc_url, additional_headers)?;
        Ok(Self { client })
    }
}

// implementing Deref in order to delegate all method calls to `rpc::Client`
impl Deref for RpcClient {
    type Target = rpc::Client;

    fn deref(&self) -> &Self::Target {
        &self.client
    }
}

// implementing DerefMut for mutable access
impl DerefMut for RpcClient {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.client
    }
}

#[cfg(test)]

mod tests {
    use super::*;

    #[test]
    fn returns_an_error_when_rpc_header_is_not_formatted_properly() {
        let rpc_header = "api key: Bearer 1234".to_string();
        let network = Network {
            rpc_url: "http://localhost:1234".to_string(),
            network_passphrase: "Network passphrase".to_string(),
            rpc_header: Some(rpc_header.clone()),
        };

        let result = RpcClient::new(network);

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            format!("invalid HTTP header name")
        );
    }

    #[test]
    fn returns_an_error_when_rpc_header_does_not_include_a_name() {
        let rpc_header = "Bearer 1234".to_string();
        let network = Network {
            rpc_url: "http://localhost:1234".to_string(),
            network_passphrase: "Network passphrase".to_string(),
            rpc_header: Some(rpc_header.clone()),
        };

        let result = RpcClient::new(network);

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            format!("Invalid header: Missing a header name and/or value: {rpc_header}")
        );
    }

    #[test]
    fn is_ok_when_the_rpc_header_is_formatted_properly() {
        let rpc_header = "Authorization: Bearer 1234".to_string();
        let network = Network {
            rpc_url: "http://localhost:1234".to_string(),
            network_passphrase: "Network passphrase".to_string(),
            rpc_header: Some(rpc_header.clone()),
        };

        let result = RpcClient::new(network);

        assert!(result.is_ok());
    }

    #[test]
    fn is_ok_when_the_rpc_header_is_lowercase() {
        let rpc_header = "authorization: bearer 1234".to_string();
        let network = Network {
            rpc_url: "http://localhost:1234".to_string(),
            network_passphrase: "Network passphrase".to_string(),
            rpc_header: Some(rpc_header.clone()),
        };

        let result = RpcClient::new(network);

        assert!(result.is_ok());
    }

    #[test]
    fn is_ok_when_there_is_no_rpc_header() {
        let network = Network {
            rpc_url: "http://localhost:1234".to_string(),
            network_passphrase: "Network passphrase".to_string(),
            rpc_header: None,
        };

        let result = RpcClient::new(network);

        assert!(result.is_ok());
    }
}
