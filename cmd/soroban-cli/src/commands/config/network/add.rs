use super::{super::secret, locator};
use clap::command;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Secret(#[from] secret::Error),

    #[error(transparent)]
    Config(#[from] locator::Error),

    #[error("Failed to write network file")]
    NetworkCreationFailed,
}

#[derive(Debug, clap::Parser)]
pub struct Cmd {
    /// Name of network
    pub name: String,

    #[command(flatten)]
    pub network: super::Network,

    #[command(flatten)]
    pub config_locator: locator::Args,
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        self.config_locator
            .write_network(&self.name, &self.network)
            .map_err(|_| Error::NetworkCreationFailed)
    }
}
