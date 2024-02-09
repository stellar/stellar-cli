use core::fmt;

use bollard::{ClientVersion, Docker};
use clap::ValueEnum;

// DEFAULT_TIMEOUT and API_DEFAULT_VERSION are from the bollard crate
const DEFAULT_TIMEOUT: u64 = 120;
const API_DEFAULT_VERSION: &ClientVersion = &ClientVersion {
    major_version: 1,
    minor_version: 40,
};

#[derive(ValueEnum, Debug, Clone, PartialEq)]
pub enum Network {
    Local,
    Testnet,
    Futurenet,
    Pubnet,
}

impl fmt::Display for Network {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let variant_str = match self {
            Network::Local => "local",
            Network::Testnet => "testnet",
            Network::Futurenet => "futurenet",
            Network::Pubnet => "pubnet",
        };

        write!(f, "{variant_str}")
    }
}

pub fn connect_to_docker(
    docker_socket_path: &Option<String>,
) -> Result<Docker, bollard::errors::Error> {
    if docker_socket_path.is_some() {
        let socket = docker_socket_path.as_ref().unwrap();
        Docker::connect_with_socket(socket, DEFAULT_TIMEOUT, API_DEFAULT_VERSION)
    } else {
        Docker::connect_with_socket_defaults()
    }
}
