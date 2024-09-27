use std::ops::{Deref, DerefMut};

use http::{HeaderMap, HeaderName, HeaderValue};

use crate::config::network::Network;
use crate::rpc;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Rpc(#[from] rpc::Error),
    #[error(transparent)]
    InvalidHeaderName(#[from] http::header::InvalidHeaderName),
    #[error(transparent)]
    InvalidHeaderValue(#[from] http::header::InvalidHeaderValue),
}

#[derive(Debug)]
pub struct RpcClient {
    client: rpc::Client,
}

impl RpcClient {
    pub fn new(network: Network) -> Result<Self, Error> {
        let mut additional_headers = HeaderMap::new();
        for header in network.rpc_headers.iter() {
            let header_name = HeaderName::from_bytes(header.0.as_bytes())?;
            let header_value = HeaderValue::from_str(&header.1)?;

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
        let network = Network {
            rpc_url: "http://localhost:1234".to_string(),
            network_passphrase: "Network passphrase".to_string(),
            rpc_headers: [("api key".to_string(), "Bearer".to_string())].to_vec(),
        };

        let result = RpcClient::new(network);

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            format!("invalid HTTP header name")
        );
    }

    #[test]
    fn is_ok_when_the_rpc_header_is_formatted_properly() {
        let network = Network {
            rpc_url: "http://localhost:1234".to_string(),
            network_passphrase: "Network passphrase".to_string(),
            rpc_headers: [("Authorization".to_string(), "Bearer 1234".to_string())].to_vec(),
        };

        let result = RpcClient::new(network);

        assert!(result.is_ok());
    }

    #[test]
    fn is_ok_when_the_rpc_header_is_lowercase() {
        let network = Network {
            rpc_url: "http://localhost:1234".to_string(),
            network_passphrase: "Network passphrase".to_string(),
            rpc_headers: [("authorization".to_string(), "bearer 1234".to_string())].to_vec(),
        };

        let result = RpcClient::new(network);

        assert!(result.is_ok());
    }

    #[test]
    fn is_ok_when_there_are_no_rpc_headers() {
        let network = Network {
            rpc_url: "http://localhost:1234".to_string(),
            network_passphrase: "Network passphrase".to_string(),
            rpc_headers: [].to_vec(),
        };

        let result = RpcClient::new(network);

        assert!(result.is_ok());
    }

    #[test]
    fn is_ok_when_there_are_several_rpc_headers() {
        let network = Network {
            rpc_url: "http://localhost:1234".to_string(),
            network_passphrase: "Network passphrase".to_string(),
            rpc_headers: [
                ("authorization".to_string(), "bearer 1234".to_string()),
                ("api-key".to_string(), "5678".to_string()),
            ]
            .to_vec(),
        };

        let result = RpcClient::new(network);

        assert!(result.is_ok());
    }
}
