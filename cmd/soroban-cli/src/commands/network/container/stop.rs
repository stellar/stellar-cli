use crate::commands::network::container::shared::{Error as BollardConnectionError, Network};

use super::shared::Args;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("⛔ Failed to connect to docker: {0}")]
    DockerConnectionFailed(#[from] BollardConnectionError),

    #[error("⛔ Container {container_name} not found")]
    ContainerNotFound {
        container_name: String,
        #[source]
        source: bollard::errors::Error,
    },

    #[error("⛔ Failed to stop container: {0}")]
    ContainerStopFailed(#[from] bollard::errors::Error),
}

#[derive(Debug, clap::Parser, Clone)]
pub struct Cmd {
    #[command(flatten)]
    pub container_args: Args,

    /// Network to stop (used in container name generation)
    #[arg(required_unless_present = "name")]
    pub network: Option<Network>,
}

impl Cmd {
    pub async fn run(&self) -> Result<(), Error> {
        let container_name = self.container_args.get_container_name(self.network);
        let docker = self.container_args.connect_to_docker().await?;
        println!("ℹ️ Stopping container: {container_name}");
        docker
            .stop_container(&container_name, None)
            .await
            .map_err(|e| {
                let msg = e.to_string();
                if msg.contains("No such container") {
                    Error::ContainerNotFound {
                        container_name: container_name.clone(),
                        source: e,
                    }
                } else {
                    Error::ContainerStopFailed(e)
                }
            })?;
        println!("✅ Container stopped: {container_name}");
        Ok(())
    }
}
