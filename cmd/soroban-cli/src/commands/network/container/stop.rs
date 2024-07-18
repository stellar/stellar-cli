use crate::commands::network::container::shared::Error as BollardConnectionError;

use super::shared::{Args, Name};

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

    /// Container to stop
    pub name: String,
}

impl Cmd {
    pub async fn run(&self) -> Result<(), Error> {
        let container_name = Name::new(Some(self.name.clone()), None);
        let docker = self.container_args.connect_to_docker().await?;
        println!(
            "ℹ️ Stopping container: {}",
            container_name.get_external_container_name()
        );
        docker
            .stop_container(&container_name.get_internal_container_name(), None)
            .await
            .map_err(|e| {
                let msg = e.to_string();
                if msg.contains("No such container") {
                    Error::ContainerNotFound {
                        container_name: container_name.get_external_container_name(),
                        source: e,
                    }
                } else {
                    Error::ContainerStopFailed(e)
                }
            })?;
        println!(
            "✅ Container stopped: {}",
            container_name.get_external_container_name()
        );
        Ok(())
    }
}
