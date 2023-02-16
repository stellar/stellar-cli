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

    /// Output the generated identity as a secret
    #[clap(long, short = 'S')]
    pub as_secret: bool,

    #[clap(flatten)]
    pub config_locator: locator::Args,

    /// When generating a secret key, which hd_path should be used from the original seed_phrase.
    #[clap(long)]
    pub hd_path: Option<usize>,
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        let seed_phrase = Secret::from_seed(self.seed.as_ref())?;
        let secret = if self.as_secret {
            let secret = seed_phrase.private_key(self.hd_path)?;
            Secret::SecretKey {
                secret_key: secret.to_string(),
            }
        } else {
            seed_phrase
        };
        self.config_locator.write_identity(&self.name, &secret)?;
        Ok(())
    }
}
