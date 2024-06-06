use chrono::offset::Local;
use futures_util::TryStreamExt;

use crate::commands::network::shared::{
    connect_to_docker, Error as ConnectionError, Network, DOCKER_HOST_HELP,
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    ConnectionError(#[from] ConnectionError),

    #[error("⛔ ️Failed to tail container: {0}")]
    TailContainerError(#[from] bollard::errors::Error),
}

#[derive(Debug, clap::Parser, Clone)]
pub struct Cmd {
    /// Network to tail
    pub network: Network,

    #[arg(short = 'd', long, help = DOCKER_HOST_HELP, env = "DOCKER_HOST")]
    pub docker_host: Option<String>,
}

impl Cmd {
    pub async fn run(&self) -> Result<(), Error> {
        let container_name = format!("stellar-{}", self.network);
        println!("ℹ️  Tailing logs for {}", container_name);
        let docker = connect_to_docker(&self.docker_host).await?;
        let logs_stream = &mut docker.logs(
            &container_name,
            Some(bollard::container::LogsOptions {
                follow: true,
                stdout: true,
                stderr: true,
                tail: "all",
                ..Default::default()
            }),
        );

        while let Some(log) = logs_stream.try_next().await? {
            print!(
                "{}: {} {}",
                container_name,
                Local::now().format("%Y-%m-%d %H:%M:%S"),
                log
            );
        }
        Ok(())
    }
}
