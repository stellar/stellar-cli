use crate::config::location;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Unknown Error")]
    Unknown,
}

#[derive(Debug, clap::Args)]
pub struct Cmd {
    /// Name of identity
    pub name: String,
    /// Optional seed to use when generating seed phrase
    #[clap(long, short = 's')]
    pub seed: Option<String>,

    #[clap(flatten)]
    pub config: location::Args,
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        println!(
            "Coming soon! This will generate a new identity named {} and a corresponding seed phrase from seed {:?}",
            self.name,
            self.seed.as_deref().unwrap_or("random")
        );
        if false {
            return Err(Error::Unknown);
        }
        Ok(())
    }
}
