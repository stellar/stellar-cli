use std::io::{IsTerminal, Write};

use sep5::SeedPhrase;

use crate::{
    commands::global,
    config::{
        address::KeyName,
        key, locator,
        secret::{self, Secret},
    },
    print::Print,
    signer::secure_store,
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Secret(#[from] secret::Error),
    #[error(transparent)]
    Key(#[from] key::Error),
    #[error(transparent)]
    Config(#[from] locator::Error),

    #[error(transparent)]
    SecureStore(#[from] secure_store::Error),

    #[error(transparent)]
    SeedPhrase(#[from] sep5::error::Error),

    #[error("secret input error")]
    PasswordRead,

    #[error("An identity with the name '{0}' already exists")]
    IdentityAlreadyExists(String),

    #[error(
        "--secure-store only supports seed phrases; \
         unset STELLAR_SECRET_KEY or provide a seed phrase instead"
    )]
    SecureStoreRequiresSeedPhrase,

    #[error("--hd-path is not valid with a secret key; secret keys cannot be derived")]
    HdPathNotSupportedForSecretKey,
}

#[derive(Debug, clap::Parser, Clone)]
#[group(skip)]
pub struct Cmd {
    /// Name of identity
    pub name: KeyName,

    #[command(flatten)]
    pub secrets: secret::Args,

    #[command(flatten)]
    pub config_locator: locator::Args,

    /// Add a public key, ed25519, or muxed account, e.g. G1.., M2..
    #[arg(
        long,
        conflicts_with = "seed_phrase",
        conflicts_with = "secret_key",
        conflicts_with = "hd_path"
    )]
    pub public_key: Option<String>,

    /// Overwrite existing identity if it already exists. When combined with
    /// --secure-store, also replaces the existing Secure Store entry.
    #[arg(long)]
    pub overwrite: bool,

    /// When importing a seed phrase, which `hd_path` to derive the key at.
    /// Persisted on the identity so later commands derive the same account
    /// without re-passing the flag. Not valid with `--public-key` or a raw
    /// secret key.
    #[arg(long)]
    pub hd_path: Option<usize>,
}

impl Cmd {
    pub fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let print = Print::new(global_args.quiet);

        if self.config_locator.read_identity(&self.name).is_ok() {
            if !self.overwrite {
                return Err(Error::IdentityAlreadyExists(self.name.to_string()));
            }

            print.exclaimln(format!("Overwriting identity '{}'", &self.name.to_string()));
        }

        let key = if let Some(key) = self.public_key.as_ref() {
            key::Key::parse_public_only(key)?
        } else {
            self.read_secret(&print)?.into()
        };

        let path = self.config_locator.write_key(&self.name, &key)?;

        print.checkln(format!("Key saved with alias {} in {path:?}", self.name));

        Ok(())
    }

    fn read_secret(&self, print: &Print) -> Result<Secret, Error> {
        if self.secrets.secure_store {
            if std::env::var("STELLAR_SECRET_KEY").is_ok() {
                return Err(Error::SecureStoreRequiresSeedPhrase);
            }
        } else if let Ok(secret_key) = std::env::var("STELLAR_SECRET_KEY") {
            return build_secret(&secret_key, self.hd_path);
        }

        if self.secrets.secure_store {
            let prompt = "Type a 12/24 word seed phrase:";
            let secret_key = read_password(print, prompt)?;
            if secret_key.split_whitespace().count() < 24 {
                print.warnln("The provided seed phrase lacks sufficient entropy and should be avoided. Using a 24-word seed phrase is a safer option.".to_string());
                print.warnln(
                    "To generate a new key, use the `stellar keys generate` command.".to_string(),
                );
            }

            let seed_phrase: SeedPhrase = secret_key.parse()?;

            Ok(secure_store::save_secret(
                print,
                &self.name,
                &seed_phrase,
                self.hd_path,
                self.overwrite,
            )?)
        } else {
            let prompt = "Type a secret key or 12/24 word seed phrase:";
            let secret_key = read_password(print, prompt)?;
            let secret = build_secret(&secret_key, self.hd_path)?;
            if let Secret::SeedPhrase { seed_phrase, .. } = &secret {
                if seed_phrase.split_whitespace().count() < 24 {
                    print.warnln("The provided seed phrase lacks sufficient entropy and should be avoided. Using a 24-word seed phrase is a safer option.".to_string());
                    print.warnln(
                        "To generate a new key, use the `stellar keys generate` command."
                            .to_string(),
                    );
                }
            }
            Ok(secret)
        }
    }
}

fn build_secret(input: &str, hd_path: Option<usize>) -> Result<Secret, Error> {
    let secret: Secret = input.parse()?;
    match (secret, hd_path) {
        (Secret::SecretKey { .. }, Some(_)) => Err(Error::HdPathNotSupportedForSecretKey),
        (Secret::SeedPhrase { seed_phrase, .. }, hd_path) => Ok(Secret::SeedPhrase {
            seed_phrase,
            hd_path,
        }),
        (secret, _) => Ok(secret),
    }
}

fn read_password(print: &Print, prompt: &str) -> Result<String, Error> {
    if std::io::stdin().is_terminal() {
        // Interactive: prompt and read from TTY
        print.arrowln(prompt);
        std::io::stdout().flush().map_err(|_| Error::PasswordRead)?;
        rpassword::read_password().map_err(|_| Error::PasswordRead)
    } else {
        // Non-interactive: read from stdin
        let mut input = String::new();
        std::io::stdin()
            .read_line(&mut input)
            .map_err(|_| Error::PasswordRead)?;
        let input = input.trim().to_string();
        if input.is_empty() {
            return Err(Error::PasswordRead);
        }
        Ok(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::key::{self as key_mod, Key};

    const PUBLIC_KEY: &str = "GAKSH6AD2IPJQELTHIOWDAPYX74YELUOWJLI2L4RIPIPZH6YQIFNUSDC";
    const MUXED_ACCOUNT: &str =
        "MA3D5KRYM6CB7OWQ6TWYRR3Z4T7GNZLKERYNZGGA5SOAOPIFY6YQGAAAAAAAAAPCICBKU";
    const SECRET_KEY: &str = "SBF5HLRREHMS36XZNTUSKZ6FTXDZGNXOHF4EXKUL5UCWZLPBX3NGJ4BH";
    const SEED_PHRASE: &str =
        "depth decade power loud smile spatial sign movie judge february rate broccoli";

    fn set_up_test() -> (tempfile::TempDir, locator::Args, Cmd) {
        let temp_dir = tempfile::tempdir().unwrap();
        let locator = locator::Args {
            config_dir: Some(temp_dir.path().to_path_buf()),
        };
        let cmd = Cmd {
            name: "test_name".parse().unwrap(),
            secrets: secret::Args {
                secret_key: false,
                seed_phrase: false,
                secure_store: false,
            },
            config_locator: locator.clone(),
            public_key: None,
            overwrite: false,
            hd_path: None,
        };
        (temp_dir, locator, cmd)
    }

    fn cmd_with_public_key(
        public_key: &str,
        hd_path: Option<usize>,
    ) -> (tempfile::TempDir, locator::Args, Cmd) {
        let (temp_dir, locator, mut cmd) = set_up_test();
        cmd.public_key = Some(public_key.to_string());
        cmd.hd_path = hd_path;
        (temp_dir, locator, cmd)
    }

    fn global_args() -> global::Args {
        global::Args {
            quiet: true,
            ..Default::default()
        }
    }

    #[test]
    fn test_build_secret_persists_hd_path_on_seed_phrase() {
        let secret = build_secret(SEED_PHRASE, Some(5)).unwrap();
        match secret {
            Secret::SeedPhrase {
                seed_phrase,
                hd_path,
            } => {
                assert_eq!(seed_phrase, SEED_PHRASE);
                assert_eq!(hd_path, Some(5));
            }
            other => panic!("expected SeedPhrase variant, got {other:?}"),
        }
    }

    #[test]
    fn test_build_secret_seed_phrase_without_hd_path() {
        let secret = build_secret(SEED_PHRASE, None).unwrap();
        match secret {
            Secret::SeedPhrase { hd_path, .. } => assert_eq!(hd_path, None),
            other => panic!("expected SeedPhrase variant, got {other:?}"),
        }
    }

    #[test]
    fn test_build_secret_rejects_hd_path_with_secret_key() {
        let result = build_secret(SECRET_KEY, Some(5));
        assert!(matches!(result, Err(Error::HdPathNotSupportedForSecretKey)));
    }

    #[test]
    fn test_build_secret_secret_key_without_hd_path() {
        let secret = build_secret(SECRET_KEY, None).unwrap();
        assert!(matches!(secret, Secret::SecretKey { .. }));
    }

    #[test]
    fn test_clap_rejects_hd_path_with_public_key() {
        // clap-level conflict: --public-key cannot be combined with --hd-path.
        // Driving through `try_parse_from` rather than constructing `Cmd`
        // directly is what exercises the conflict.
        use clap::Parser;

        let result = Cmd::try_parse_from([
            "add",
            "test_name",
            "--public-key",
            PUBLIC_KEY,
            "--hd-path",
            "3",
        ]);
        let err = result.expect_err("clap must reject --public-key + --hd-path");
        assert_eq!(err.kind(), clap::error::ErrorKind::ArgumentConflict);
    }

    #[test]
    fn test_run_accepts_public_key_without_hd_path() {
        let (_tmp, _locator, cmd) = cmd_with_public_key(PUBLIC_KEY, None);
        assert!(cmd.run(&global_args()).is_ok());
    }

    #[test]
    fn public_key_flag_accepts_public_key() {
        let (_tmp, locator, mut cmd) = set_up_test();
        cmd.public_key = Some(PUBLIC_KEY.to_string());
        cmd.run(&global_args()).unwrap();
        let stored = locator.read_identity("test_name").unwrap();
        assert!(matches!(stored, Key::PublicKey(_)));
    }

    #[test]
    fn public_key_flag_accepts_muxed_account() {
        let (_tmp, locator, mut cmd) = set_up_test();
        cmd.public_key = Some(MUXED_ACCOUNT.to_string());
        cmd.run(&global_args()).unwrap();
        let stored = locator.read_identity("test_name").unwrap();
        assert!(matches!(stored, Key::MuxedAccount(_)));
    }

    #[test]
    fn public_key_flag_rejects_secret_key() {
        let (_tmp, locator, mut cmd) = set_up_test();
        cmd.public_key = Some(SECRET_KEY.to_string());
        let err = cmd.run(&global_args()).unwrap_err();
        assert!(matches!(err, Error::Key(key_mod::Error::PublicKeyExpected)));
        assert!(locator.read_identity("test_name").is_err());
    }

    #[test]
    fn public_key_flag_rejects_seed_phrase() {
        let (_tmp, locator, mut cmd) = set_up_test();
        cmd.public_key = Some(SEED_PHRASE.to_string());
        let err = cmd.run(&global_args()).unwrap_err();
        assert!(matches!(err, Error::Key(key_mod::Error::PublicKeyExpected)));
        assert!(locator.read_identity("test_name").is_err());
    }

    #[test]
    fn public_key_flag_rejects_ledger() {
        let (_tmp, locator, mut cmd) = set_up_test();
        cmd.public_key = Some("ledger".to_string());
        let err = cmd.run(&global_args()).unwrap_err();
        assert!(matches!(err, Error::Key(key_mod::Error::PublicKeyExpected)));
        assert!(locator.read_identity("test_name").is_err());
    }

    #[test]
    fn public_key_flag_rejects_secure_store() {
        let (_tmp, locator, mut cmd) = set_up_test();
        cmd.public_key = Some("secure_store:org.stellar.cli-alice".to_string());
        let err = cmd.run(&global_args()).unwrap_err();
        assert!(matches!(err, Error::Key(key_mod::Error::PublicKeyExpected)));
        assert!(locator.read_identity("test_name").is_err());
    }
}
