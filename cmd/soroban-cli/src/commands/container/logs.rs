use crate::commands::{container::shared::Error as ConnectionError, global};

use super::shared::{Args, Name};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Docker(#[from] ConnectionError),

    #[error("failed to tail container logs")]
    TailContainerError,
}

#[derive(Debug, clap::Parser, Clone)]
pub struct Cmd {
    #[command(flatten)]
    pub container_args: Args,

    /// Container to get logs from
    #[arg(default_value = "local")]
    pub name: String,
}

impl Cmd {
    pub async fn run(&self, _global_args: &global::Args) -> Result<(), Error> {
        let container_name = Name(self.name.clone()).get_internal_container_name();

        // Stream logs straight to the terminal by inheriting stdio.
        let status = self
            .container_args
            .docker_command()
            .args(["logs", "-f", "--tail", "all", &container_name])
            .status()
            .await
            .map_err(ConnectionError::from)?;

        if !status.success() {
            return Err(Error::TailContainerError);
        }

        Ok(())
    }
}
