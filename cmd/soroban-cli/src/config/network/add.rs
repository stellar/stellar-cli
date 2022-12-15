use crate::config::{args, secret};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Secret(#[from] secret::Error),

    #[error(transparent)]
    Config(#[from] args::Error),

    #[error("Failed to write network file")]
    IdCreationFailed,
}

#[derive(Debug, clap::Args)]
pub struct Cmd {
    /// Name of network
    pub name: String,

    #[clap(flatten)]
    pub secrets: secret::Args,

    /// Set as default network
    #[clap(long)]
    pub default: bool,

    #[clap(flatten)]
    pub config: args::Args,
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        self.config
            .write_network(&self.name, &self.secrets.read_secret()?)
            .map_err(|_| Error::IdCreationFailed)
    }
}
