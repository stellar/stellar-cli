use clap::{arg, command};

use super::super::config::{
    locator, network,
    secret::{self, Secret},
};
use crate::{commands::global, print::Print};

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
#[allow(clippy::struct_excessive_bools)]
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

    /// Fund generated key pair
    #[arg(long, default_value = "false")]
    pub fund: bool,

    /// Overwrite existing identity if it already exists.
    #[arg(long)]
    pub overwrite: bool,
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let print = Print::new(global_args.quiet);

        if self.config_locator.read_identity(&self.name).is_ok() {
            if !self.overwrite {
                return Err(Error::IdentityAlreadyExists(self.name.clone()));
            }

            print.exclaimln(format!("Overwriting identity '{}'", &self.name));
        }

        if !self.fund {
            print.warnln(
                "Behavior of `generate` will change in the \
            future, and it will no longer fund by default. If you want to fund please \
            provide `--fund` flag. If you don't need to fund your keys in the future, ignore this \
            warning. It can be suppressed with -q flag.",
            );
        }

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

        let path = self.config_locator.write_identity(&self.name, &secret)?;
        print.checkln(format!("Key saved with alias {:?} in {path:?}", self.name));

        if !self.no_fund {
            let addr = secret.public_key(self.hd_path)?;
            let network = self.network.get(&self.config_locator)?;
            network
                .fund_address(&addr)
                .await
                .map_err(|e| {
                    tracing::warn!("fund_address failed: {e}");
                })
                .unwrap_or_default();
            print.checkln(format!(
                "Account {:?} funded on {:?}",
                self.name, network.network_passphrase
            ));
        }

        Ok(())
    }
}
