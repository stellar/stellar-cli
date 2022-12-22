use crate::config::locator;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Location(#[from] locator::Error),
}

#[derive(Debug, clap::Args)]
pub struct Cmd {
    /// default name
    pub default_name: String,

    #[clap(flatten)]
    pub config_locator: locator::Args,
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        Ok(self
            .config_locator
            .set_default_identity(&self.default_name)?)
    }
}
