use std::ops::{Deref, DerefMut};

use http::{HeaderMap, HeaderName, HeaderValue};

use crate::config::network::Network;
use crate::rpc;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Rpc(#[from] rpc::Error),
}

// todo:
// allow the user to pass in multiple headers
// make sure that this will work if the header is all lowercase, etc
// move the creation of the HeaderMap into the rpc client fn instead?
// make sure that there are 2 header components before continuing
// refactor all the things to use a wrapped rpc client so that i just have to make this change once

// this can be a wrap around the stellar-rpc-client
// is there a way to delegate all the calls to the stellar-rpc-client?
pub struct RpcClient {
    client: rpc::Client,
}

impl RpcClient {
    pub fn new(network: Network) -> Result<Self, Error> {
        let mut additional_headers = HeaderMap::new();
        if let Some(rpc_header) = network.rpc_header {
            let header_components = rpc_header.split(":").collect::<Vec<&str>>();
            let key = header_components[0];
            let value = header_components[1];

            let header_name = HeaderName::from_bytes(key.as_bytes()).expect("Invalid header name");
            let header_value = HeaderValue::from_str(value).expect("Invalid header value");

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
