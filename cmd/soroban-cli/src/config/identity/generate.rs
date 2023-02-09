use crate::config::{
    locator,
    secret::{self, Secret},
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Config(#[from] locator::Error),
    #[error(transparent)]
    Secret(#[from] secret::Error),
}

#[derive(Debug, clap::Args)]
pub struct Cmd {
    /// Name of identity
    pub name: String,
    /// Optional seed to use when generating seed phrase.
    /// Random otherwise.
    #[clap(long, short = 's')]
    pub seed: Option<String>,

    #[clap(flatten)]
    pub config_locator: locator::Args,
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        let secret = Secret::from_seed(self.seed.as_ref())?;
        self.config_locator.write_identity(&self.name, &secret)?;
        Ok(())
    }
}
