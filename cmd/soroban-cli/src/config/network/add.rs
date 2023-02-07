use crate::config::{locator, secret};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Secret(#[from] secret::Error),

    #[error(transparent)]
    Config(#[from] locator::Error),

    #[error("Failed to write network file")]
    NetworkCreationFailed,
}

#[derive(Debug, clap::Args)]
pub struct Cmd {
    /// Name of network
    pub name: String,

    #[clap(flatten)]
    pub network: super::Network,

    /// Set as default network
    #[clap(long)]
    pub default: bool,

    #[clap(flatten)]
    pub config_locator: locator::Args,
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        self.config_locator
            .write_network(&self.name, &self.network)
            .map_err(|_| Error::NetworkCreationFailed)
    }
}
