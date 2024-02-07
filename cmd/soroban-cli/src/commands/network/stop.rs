use core::fmt;

use bollard::{ClientVersion, Docker};
use clap::ValueEnum;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Failed to stop container: {0}")]
    StopContainerError(#[from] bollard::errors::Error),
}

// DEFAULT_TIMEOUT and API_DEFAULT_VERSION are from the bollard crate
const DEFAULT_TIMEOUT: u64 = 120;
const API_DEFAULT_VERSION: &ClientVersion = &ClientVersion {
    major_version: 1,
    minor_version: 40,
};

// TODO: move to a shared module
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

        write!(f, "{}", variant_str)
    }
}

#[derive(Debug, clap::Parser, Clone)]
pub struct Cmd {
    /// network container to stop
    pub network: Network,

    /// optional argument to override the default docker socket path
    #[arg(short = 'd', long)]
    pub docker_socket_path: Option<String>,
}

impl Cmd {
    pub async fn run(&self) -> Result<(), Error> {
        let container_name = format!("stellar-{}", self.network);
        let docker = connect_to_docker(self);
        println!("Stopping container: {container_name}");
        docker.stop_container(&container_name, None).await.unwrap();

        Ok(())
    }
}

//TODO: move to a shared module
fn connect_to_docker(cmd: &Cmd) -> Docker {
    if cmd.docker_socket_path.is_some() {
        let socket = cmd.docker_socket_path.as_ref().unwrap();
        Docker::connect_with_socket(socket, DEFAULT_TIMEOUT, API_DEFAULT_VERSION).unwrap()
    } else {
        Docker::connect_with_socket_defaults().unwrap()
    }
}
