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

    /// Save in `keychain`
    #[arg(long)]
    pub keychain: bool,

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

const KEYCHAIN_CONSTANT: &str = "keychain";

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let print = Print::new(global_args.quiet);

        if self.config_locator.read_identity(&self.name).is_ok() {
            if !self.overwrite {
                return Err(Error::IdentityAlreadyExists(self.name.to_string()));
            }

            print.exclaimln(format!("Overwriting identity '{}'", &self.name.to_string()));
        }

        if !self.fund {
            print.warnln(
                "Behavior of `generate` will change in the \
            future, and it will no longer fund by default. If you want to fund please \
            provide `--fund` flag. If you don't need to fund your keys in the future, ignore this \
            warning. It can be suppressed with -q flag.",
            );
        }
        let secret = self.secret()?;
        self.config_locator.write_identity(&self.name, &secret)?;

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
        }

        Ok(())
    }

    fn secret(&self) -> Result<Secret, Error> {
        let seed_phrase = self.seed_phrase()?;
        Ok(if self.as_secret {
            seed_phrase.private_key(self.hd_path)?.into()
        } else if self.keychain {
            KEYCHAIN_CONSTANT.parse()?
        } else {
            seed_phrase
        })
    }

    fn seed_phrase(&self) -> Result<Secret, Error> {
        Ok(if self.default_seed {
            Secret::test_seed_phrase()
        } else {
            Secret::from_seed(self.seed.as_deref())
        }?)
    }
}

#[cfg(test)]
mod tests {
    use crate::config::secret::Secret;

    fn set_up_test() -> (super::locator::Args, super::Cmd) {
        let temp_dir = tempfile::tempdir().unwrap();
        let locator = super::locator::Args {
            global: false,
            config_dir: Some(temp_dir.path().to_path_buf()),
        };

        let cmd = super::Cmd {
            name: "test_name".to_string(),
            no_fund: true,
            seed: None,
            as_secret: false,
            keychain: false,
            config_locator: locator.clone(),
            hd_path: None,
            default_seed: false,
            network: Default::default(),
            fund: false,
        };

        (locator, cmd)
    }

    fn global_args() -> super::global::Args {
        let mut global_args = super::global::Args::default();
        global_args.quiet = true;
        global_args
    }

    #[tokio::test]
    async fn test_storing_secret_as_a_seed_phrase() {
        let (test_locator, cmd) = set_up_test();
        let global_args = global_args();

        let result = cmd.run(&global_args).await;
        assert!(result.is_ok());
        let identity = test_locator.read_identity("test_name").unwrap();
        assert!(matches!(identity, Secret::SeedPhrase { .. }));
    }

    #[tokio::test]
    async fn test_storing_secret_as_a_secret_key() {
        let (test_locator, mut cmd) = set_up_test();
        cmd.as_secret = true;
        let global_args = global_args();

        let result = cmd.run(&global_args).await;
        assert!(result.is_ok());
        let identity = test_locator.read_identity("test_name").unwrap();
        assert!(matches!(identity, Secret::SecretKey { .. }));
    }

    #[tokio::test]
    async fn test_storing_secret_in_keychain() {
        let (test_locator, mut cmd) = set_up_test();
        cmd.keychain = true;
        let global_args = global_args();

        let result = cmd.run(&global_args).await;
        assert!(result.is_ok());
        let identity = test_locator.read_identity("test_name").unwrap();
        assert!(matches!(identity, Secret::Keychain { .. }));
    }
}
