use clap::{arg, command};
use sep5::SeedPhrase;

use super::super::config::{
    locator, network,
    secret::{self, Secret},
};

use crate::{commands::global, config::address::KeyName, print::Print, signer::secure_store};

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

    #[error(transparent)]
    SecureStore(#[from] secure_store::Error),
}

#[derive(Debug, clap::Parser, Clone)]
#[group(skip)]
#[allow(clippy::struct_excessive_bools)]
pub struct Cmd {
    /// Name of identity
    pub name: KeyName,

    /// Optional seed to use when generating seed phrase.
    /// Random otherwise.
    #[arg(long)]
    pub seed: Option<String>,

    /// Output the generated identity as a secret key
    #[arg(long, short = 's')]
    pub as_secret: bool,

    /// Save the new key in your OS's credential secure store.
    ///
    /// On Mac this uses Keychain, on Windows it is Secure Store Service, and on *nix platforms it uses a combination of the kernel keyutils and DBus-based Secret Service.
    #[arg(long)]
    pub secure_store: bool,

    #[command(flatten)]
    pub config_locator: locator::Args,

    /// When generating a secret key, which `hd_path` should be used from the original `seed_phrase`.
    #[arg(long)]
    pub hd_path: Option<usize>,

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
                return Err(Error::IdentityAlreadyExists(self.name.to_string()));
            }

            print.exclaimln(format!("Overwriting identity '{}'", &self.name.to_string()));
        }

        let secret = self.secret(&print)?;
        let path = self.config_locator.write_identity(&self.name, &secret)?;
        print.checkln(format!("Key saved with alias {} in {path:?}", self.name));

        if self.fund {
            self.fund(&secret, &print).await?;
        }

        Ok(())
    }

    async fn fund(&self, secret: &Secret, print: &Print) -> Result<(), Error> {
        let addr = secret.public_key(self.hd_path)?;
        let network = self.network.get(&self.config_locator)?;
        let formatted_name = self.name.to_string();

        match network.fund_address(&addr).await {
            Ok(()) => print.checkln(format!(
                "Account {} funded on {:?}",
                formatted_name, network.network_passphrase
            )),
            Err(e) => {
                tracing::trace!("Account funding error: {:?}", e);

                print.errorln(format!(
                    "Unable to fund account {} on {:?}",
                    formatted_name, network.network_passphrase
                ));
            }
        }

        Ok(())
    }

    fn secret(&self, print: &Print) -> Result<Secret, Error> {
        let seed_phrase = self.seed_phrase()?;
        if self.secure_store {
            let secret = secure_store::save_secret(print, &self.name, &seed_phrase)?;
            Ok(secret.parse()?)
        } else if self.as_secret {
            let secret: Secret = seed_phrase.into();
            Ok(secret.private_key(self.hd_path)?.into())
        } else {
            Ok(seed_phrase.into())
        }
    }

    fn seed_phrase(&self) -> Result<SeedPhrase, Error> {
        Ok(secret::seed_phrase_from_seed(self.seed.as_deref())?)
    }
}

#[cfg(test)]
mod tests {
    use crate::config::{address::KeyName, key::Key, secret::Secret};

    fn set_up_test() -> (super::locator::Args, super::Cmd) {
        let temp_dir = tempfile::tempdir().unwrap();
        let locator = super::locator::Args {
            global: false,
            config_dir: Some(temp_dir.path().to_path_buf()),
        };

        let cmd = super::Cmd {
            name: KeyName("test_name".to_string()),
            seed: None,
            as_secret: false,
            secure_store: false,
            config_locator: locator.clone(),
            hd_path: None,
            network: super::network::Args::default(),
            fund: false,
            overwrite: false,
        };

        (locator, cmd)
    }

    fn global_args() -> super::global::Args {
        super::global::Args {
            quiet: true,
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn test_storing_secret_as_a_seed_phrase() {
        let (test_locator, cmd) = set_up_test();
        let global_args = global_args();

        let result = cmd.run(&global_args).await;
        assert!(result.is_ok());
        let identity = test_locator.read_identity("test_name").unwrap();
        assert!(matches!(identity, Key::Secret(Secret::SeedPhrase { .. })));
    }

    #[tokio::test]
    async fn test_storing_secret_as_a_secret_key() {
        let (test_locator, mut cmd) = set_up_test();
        cmd.as_secret = true;
        let global_args = global_args();

        let result = cmd.run(&global_args).await;
        assert!(result.is_ok());
        let identity = test_locator.read_identity("test_name").unwrap();
        assert!(matches!(identity, Key::Secret(Secret::SecretKey { .. })));
    }

    #[cfg(feature = "additional-libs")]
    #[tokio::test]
    async fn test_storing_secret_in_secure_store() {
        use keyring::{mock, set_default_credential_builder};
        set_default_credential_builder(mock::default_credential_builder());
        let (test_locator, mut cmd) = set_up_test();
        cmd.secure_store = true;
        let global_args = global_args();

        let result = cmd.run(&global_args).await;
        assert!(result.is_ok());
        let identity = test_locator.read_identity("test_name").unwrap();
        assert!(matches!(identity, Key::Secret(Secret::SecureStore { .. })));
    }

    #[cfg(not(feature = "additional-libs"))]
    #[tokio::test]
    async fn test_storing_in_secure_store_returns_error_when_additional_libs_not_enabled() {
        let (test_locator, mut cmd) = set_up_test();
        cmd.secure_store = true;
        let global_args = global_args();

        let result = cmd.run(&global_args).await;
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            format!("Secure Store keys are not allowed: additional-libs feature must be enabled")
        );

        let identity_result = test_locator.read_identity("test_name");
        assert!(identity_result.is_err());
        assert_eq!(
            identity_result.unwrap_err().to_string(),
            format!("Failed to find config identity for test_name")
        );
    }
}
