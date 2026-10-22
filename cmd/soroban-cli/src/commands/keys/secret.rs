use crate::config::{
    key::{self, Key},
    locator,
    secret::Secret,
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Config(#[from] locator::Error),

    #[error(transparent)]
    Key(#[from] key::Error),

    #[error("identity is not tied to a seed phrase")]
    UnknownSeedPhrase,
}

#[derive(Debug, clap::Parser, Clone)]
#[group(skip)]
#[command(name = "secret", alias = "show")]
pub struct Cmd {
    /// Name of identity to lookup, default is test identity. Required unless --ledger is provided.
    #[arg(required_unless_present = "ledger")]
    pub name: Option<String>,

    /// Output seed phrase instead of private key
    #[arg(long, conflicts_with = "hd_path")]
    pub phrase: bool,

    /// If identity is a seed phrase use this hd path, default is 0
    #[arg(long, conflicts_with = "phrase")]
    pub hd_path: Option<u32>,

    /// Look up secret for a Ledger device (unsupported; Ledger devices do not reveal secret keys)
    #[arg(long, conflicts_with = "name")]
    pub ledger: bool,

    #[command(flatten)]
    pub locator: locator::Args,
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        if self.phrase {
            println!("{}", self.seed_phrase()?);
        } else {
            println!("{}", self.private_key()?.as_unredacted());
        }

        Ok(())
    }

    pub fn seed_phrase(&self) -> Result<String, Error> {
        let key = self.read_key()?;

        if let Key::Secret(Secret::SeedPhrase { seed_phrase, .. }) = key {
            Ok(seed_phrase)
        } else {
            Err(Error::UnknownSeedPhrase)
        }
    }

    pub fn private_key(&self) -> Result<stellar_strkey::ed25519::PrivateKey, Error> {
        Ok(self.read_key()?.private_key(self.hd_path)?)
    }

    fn read_key(&self) -> Result<Key, Error> {
        if self.ledger {
            return Err(Error::Key(key::Error::Secret(
                crate::config::secret::Error::LedgerDoesNotRevealSecretKey,
            )));
        }

        let name = self
            .name
            .as_deref()
            .expect("clap requires `name` unless --ledger is set");

        match self.locator.read_identity(name) {
            Ok(key) => Ok(key),
            Err(_) if name.eq_ignore_ascii_case("ledger") => Err(Error::Key(key::Error::Secret(
                crate::config::secret::Error::LedgerDoesNotRevealSecretKey,
            ))),
            Err(e) => Err(Error::Config(e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_secret_ledger_name_returns_ledger_does_not_reveal_error() {
        let cmd = Cmd {
            name: Some("ledger".to_string()),
            phrase: false,
            hd_path: None,
            ledger: false,
            locator: locator::Args::default(),
        };
        let err = cmd.private_key().unwrap_err();
        assert!(matches!(
            err,
            Error::Key(key::Error::Secret(
                crate::config::secret::Error::LedgerDoesNotRevealSecretKey
            ))
        ));
    }

    #[test]
    fn test_secret_ledger_name_phrase_returns_ledger_does_not_reveal_error() {
        let cmd = Cmd {
            name: Some("ledger".to_string()),
            phrase: true,
            hd_path: None,
            ledger: false,
            locator: locator::Args::default(),
        };
        let err = cmd.seed_phrase().unwrap_err();
        assert!(matches!(
            err,
            Error::Key(key::Error::Secret(
                crate::config::secret::Error::LedgerDoesNotRevealSecretKey
            ))
        ));
    }

    #[test]
    fn test_secret_ledger_flag_returns_ledger_does_not_reveal_error() {
        let cmd = Cmd {
            name: None,
            phrase: false,
            hd_path: None,
            ledger: true,
            locator: locator::Args::default(),
        };
        let err = cmd.private_key().unwrap_err();
        assert!(matches!(
            err,
            Error::Key(key::Error::Secret(
                crate::config::secret::Error::LedgerDoesNotRevealSecretKey
            ))
        ));
    }

    #[test]
    fn test_secret_clap_parsing() {
        let cmd =
            Cmd::try_parse_from(["secret", "--ledger"]).expect("--ledger parses without name");
        assert!(cmd.ledger);
        assert_eq!(cmd.name, None);

        let cmd2 = Cmd::try_parse_from(["secret", "alice"]).expect("name parses without --ledger");
        assert_eq!(cmd2.name, Some("alice".to_string()));
        assert!(!cmd2.ledger);

        let err = Cmd::try_parse_from(["secret", "alice", "--ledger"])
            .expect_err("name and --ledger conflict");
        let err_msg = err.to_string();
        assert!(err_msg.contains("cannot be used with") || err_msg.contains("conflict"));
    }
}
