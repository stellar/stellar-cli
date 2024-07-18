use futures_util::TryStreamExt;

use crate::commands::network::container::shared::Error as ConnectionError;

use super::shared::{Args, Name};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    ConnectionError(#[from] ConnectionError),

    #[error("⛔ ️Failed to tail container: {0}")]
    TailContainerError(#[from] bollard::errors::Error),
}

#[derive(Debug, clap::Parser, Clone)]
pub struct Cmd {
    #[command(flatten)]
    pub container_args: Args,

    /// Container to get logs from
    pub name: String,
}

impl Cmd {
    pub async fn run(&self) -> Result<(), Error> {
        let container_name = Name::new(self.name.clone()).get_internal_container_name();
        let docker = self.container_args.connect_to_docker().await?;
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
            print!("{log}");
        }
        Ok(())
    }
}
