use clap::{arg, command};

use super::super::config::{
    locator, network,
    secret::{self, Secret},
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Config(#[from] locator::Error),
    #[error(transparent)]
    Secret(#[from] secret::Error),
    #[error(transparent)]
    Network(#[from] network::Error),
    #[error("An identity with the name '{0}' already exists")]
    IdentityAlreadyExists(String),
}

#[derive(Debug, clap::Parser, Clone)]
#[group(skip)]
pub struct Cmd {
    /// Name of identity
    pub name: String,
    /// Optional seed to use when generating seed phrase.
    /// Random otherwise.
    #[arg(long, conflicts_with = "default_seed")]
    pub seed: Option<String>,

    /// Output the generated identity as a secret key
    #[arg(long, short = 's')]
    pub as_secret: bool,

    #[command(flatten)]
    pub config_locator: locator::Args,

    /// When generating a secret key, which `hd_path` should be used from the original `seed_phrase`.
    #[arg(long)]
    pub hd_path: Option<usize>,

    /// Generate the default seed phrase. Useful for testing.
    /// Equivalent to --seed 0000000000000000
    #[arg(long, short = 'd', conflicts_with = "seed")]
    pub default_seed: bool,

    /// Overwrite existing identity if it already exists
    #[arg(long)]
    pub overwrite: bool,

    #[command(flatten)]
    pub network: network::Args,
}

impl Cmd {
    fn write_identity(&self, secret: &Secret) -> Result<(), Error> {
        self.config_locator.write_identity(&self.name, secret)?;
        eprintln!("✅ Generated new key for '{}'", self.name);
        eprintln!("ℹ️ Public key: {}", secret.public_key(self.hd_path)?);
        eprintln!(
            "ℹ️ Secret key: hidden (use 'stellar keys show {}' to view)",
            self.name
        );
        Ok(())
    }

    fn handle_existing_identity(&self) {
        eprintln!("The identity {} already exists!", self.name);
        if let Ok(root) = self.config_locator.config_dir() {
            let mut path = root.join("identity").join(&self.name);
            path.set_extension("toml");
            eprintln!("    Seed phrase found at: {}", path.display());
        }
    }

    pub fn run(&self) -> Result<(), Error> {
        let seed_phrase = if self.default_seed {
            Secret::test_seed_phrase()
        } else {
            Secret::from_seed(self.seed.as_deref())
        }?;
        let secret = if self.as_secret {
            seed_phrase.private_key(self.hd_path)?.into()
        } else {
            seed_phrase
        };

        // Check if identity exists
        if let Ok(_existing_secret) = self.config_locator.read_identity(&self.name) {
            if self.overwrite {
                eprintln!(
                    "Overwriting existing identity '{}' as requested.",
                    self.name
                );
                self.write_identity(&secret)?;
            } else {
                self.handle_existing_identity();
                return Err(Error::IdentityAlreadyExists(self.name.clone()));
            }
        } else {
            // Identity doesn't exist, create new one
            self.write_identity(&secret)?;
        }

        Ok(())
    }
}
