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
}

#[derive(Debug, clap::Parser, Clone)]
#[group(skip)]
pub struct Cmd {
    /// Name of identity
    pub name: String,
    /// Do not fund address
    #[arg(long)]
    pub no_fund: bool,
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

    #[command(flatten)]
    pub network: network::Args,
}

impl Cmd {
    async fn fund_identity(&self, secret: &Secret) -> Result<(), Error> {
        if !self.no_fund {
            let addr = secret.public_key(self.hd_path)?;
            let network = self.network.get(&self.config_locator)?;
            eprintln!("🌎 Funding account with public key as address on testnet...");
            match network.fund_address(&addr).await {
                Ok(()) => {
                    eprintln!("✅ Funded (use 'stellar keys fund me' to fund again)");
                }
                Err(e) => {
                    tracing::warn!("fund_address failed: {e}");
                }
            }
        }
        Ok(())
    }

    async fn write_and_fund_identity(&self, secret: &Secret) -> Result<(), Error> {
        self.config_locator.write_identity(&self.name, secret)?;
        eprintln!("✅ Generated new key for '{}'", self.name);
        eprintln!("ℹ️ Public key: {}", secret.public_key(self.hd_path)?);
        eprintln!(
            "ℹ️ Secret key: hidden (use 'stellar keys show {}' to view)",
            self.name
        );
        self.fund_identity(secret).await?;
        Ok(())
    }

    async fn handle_existing_identity(&self, existing_secret: &Secret) -> Result<(), Error> {
        eprintln!("The identity {} already exists!", self.name);
        if let Ok(root) = self.config_locator.config_dir() {
            let mut path = root.join("identity").join(&self.name);
            path.set_extension("toml");
            eprintln!("    Seed phrase found at: {}", path.display());
        }
        self.fund_identity(existing_secret).await?;
        Ok(())
    }

    pub async fn run(&self) -> Result<(), Error> {
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
        match self.config_locator.read_identity(&self.name) {
            Ok(existing_secret) => {
                if self.seed.is_some() || self.default_seed {
                    // Compare secrets only if seed is provided
                    match (
                        existing_secret.private_key(self.hd_path),
                        secret.private_key(self.hd_path),
                    ) {
                        (Ok(existing_pk), Ok(new_pk)) if existing_pk == new_pk => {
                            self.handle_existing_identity(&existing_secret).await?;
                            return Ok(());
                        }
                        _ => {
                            // Secrets don't match
                            eprintln!("An identity with the name {} already exists but has a different secret. Overwriting...", self.name);
                            self.write_and_fund_identity(&secret).await?;
                        }
                    }
                } else {
                    // No seed provided, inform user that identity already exists
                    self.handle_existing_identity(&existing_secret).await?;
                    return Ok(());
                }
            }
            Err(_) => {
                // Identity doesn't exist, create new one
                self.write_and_fund_identity(&secret).await?;
            }
        }
        Ok(())
    }
}
