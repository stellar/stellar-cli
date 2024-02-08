use crate::commands::network::shared::connect_to_docker;
use crate::commands::network::shared::Network;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Failed to stop container: {0}")]
    StopContainerError(#[from] bollard::errors::Error),
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
        let docker = connect_to_docker(&self.docker_socket_path);
        println!("Stopping container: {container_name}");
        docker.stop_container(&container_name, None).await.unwrap();

        Ok(())
    }
}
