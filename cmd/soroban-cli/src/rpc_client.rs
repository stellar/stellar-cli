use std::collections::HashMap;
use std::ops::{Deref, DerefMut};

use jsonrpsee_http_client::HeaderMap;

use crate::config::network::Network;
use crate::rpc;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Rpc(#[from] rpc::Error),
    #[error("invalid HTTP header: {0}")]
    InvalidHeader(String),
}

#[derive(Debug)]
pub struct RpcClient {
    client: rpc::Client,
}

impl RpcClient {
    pub fn new(network: &Network) -> Result<Self, Error> {
        let mut header_hash_map = HashMap::new();
        for (header_name, header_value) in &network.rpc_headers {
            header_hash_map.insert(header_name.to_string(), header_value.to_string());
        }

        let header_map: HeaderMap = (&header_hash_map)
            .try_into()
            .map_err(|e| Error::InvalidHeader(format!("{:?}", e)))?;

        let client = rpc::Client::new_with_headers(&network.rpc_url, header_map)?;
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

        let result = RpcClient::new(&network);

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            format!("invalid HTTP header: http::Error(InvalidHeaderName)")
        );
    }

    #[test]
    fn is_ok_when_the_rpc_header_is_formatted_properly() {
        let network = Network {
            rpc_url: "http://localhost:1234".to_string(),
            network_passphrase: "Network passphrase".to_string(),
            rpc_headers: [("Authorization".to_string(), "Bearer 1234".to_string())].to_vec(),
        };

        let result = RpcClient::new(&network);

        assert!(result.is_ok());
    }

    #[test]
    fn is_ok_when_the_rpc_header_is_lowercase() {
        let network = Network {
            rpc_url: "http://localhost:1234".to_string(),
            network_passphrase: "Network passphrase".to_string(),
            rpc_headers: [("authorization".to_string(), "bearer 1234".to_string())].to_vec(),
        };

        let result = RpcClient::new(&network);

        assert!(result.is_ok());
    }

    #[test]
    fn is_ok_when_there_are_no_rpc_headers() {
        let network = Network {
            rpc_url: "http://localhost:1234".to_string(),
            network_passphrase: "Network passphrase".to_string(),
            rpc_headers: [].to_vec(),
        };

        let result = RpcClient::new(&network);

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

        let result = RpcClient::new(&network);

        assert!(result.is_ok());
    }
}
