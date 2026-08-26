use crate::{
    commands::{container::shared::Error as ConnectionError, global},
    print,
};

use super::shared::{Args, Name};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Docker(#[from] ConnectionError),

    #[error("container {container_name} not found")]
    ContainerNotFound { container_name: String },

    #[error("failed to stop container: {0}")]
    ContainerStopFailed(String),
}

#[derive(Debug, clap::Parser, Clone)]
pub struct Cmd {
    #[command(flatten)]
    pub container_args: Args,

    /// Container to stop
    #[arg(default_value = "local")]
    pub name: String,
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let print = print::Print::new(global_args.quiet);
        let container_name = Name(self.name.clone());

        print.infoln(format!(
            "Stopping {} container",
            container_name.get_external_container_name()
        ));

        self.container_args.warn_if_host_ignored(&print);

        let output = self
            .container_args
            .stop_command(&container_name.get_internal_container_name())
            .output()
            .await
            .map_err(|e| self.container_args.io_error(e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if self.container_args.is_container_not_found(&stderr) {
                return Err(Error::ContainerNotFound {
                    container_name: container_name.get_external_container_name(),
                });
            }
            return Err(Error::ContainerStopFailed(stderr.trim().to_string()));
        }

        print.checkln("Container stopped");

        Ok(())
    }
}
