use crate::config::location;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Location(#[from] location::Error),
}

#[derive(Debug, clap::Args)]
pub struct Cmd {
    /// default name
    pub default_name: String,

    #[clap(flatten)]
    pub config: location::Args,
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        Ok(self.config.set_default_identity(&self.default_name)?)
    }
}
