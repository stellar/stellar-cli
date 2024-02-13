use crate::commands::network::shared::{connect_to_docker, Network, DOCKER_SOCKET_PATH_HELP};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Failed to stop container: {0}")]
    StopContainerError(#[from] bollard::errors::Error),
}

#[derive(Debug, clap::Parser, Clone)]
pub struct Cmd {
    /// Network to stop
    pub network: Network,

    #[arg(short = 'd', long, help = DOCKER_SOCKET_PATH_HELP)]
    pub docker_socket_path: Option<String>,
}

impl Cmd {
    pub async fn run(&self) -> Result<(), Error> {
        let container_name = format!("stellar-{}", self.network);
        let docker = connect_to_docker(&self.docker_socket_path).await?;
        println!("ℹ️  Stopping container: {container_name}");
        docker.stop_container(&container_name, None).await.unwrap();
        println!("✅ Container stopped: {container_name}");
        Ok(())
    }
}
