use crate::commands::config::secret;

use super::super::config::locator;
use clap::arg;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Config(#[from] locator::Error),

    #[error(transparent)]
    Secret(#[from] secret::Error),

    #[error(transparent)]
    StrKey(#[from] stellar_strkey::DecodeError),
    #[error(transparent)]
    Ledger(#[from] stellar_ledger::LedgerError),
    #[error("Invalid HD path index {0}")]
    UsizeConversionError(usize),
}

#[derive(Debug, clap::Parser, Clone)]
#[group(skip)]
pub struct Cmd {
    /// Name of identity to lookup, default test identity used if not provided
    pub name: Option<String>,

    /// Use Ledger
    #[arg(long, short = 'l')]
    pub use_ledger: bool,

    /// If identity is a seed phrase use this hd path, default is 0
    #[arg(long)]
    pub hd_path: Option<usize>,

    #[command(flatten)]
    pub locator: locator::Args,
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        println!("{}", self.public_key()?);
        Ok(())
    }

    pub fn name(&self) -> &str {
        self.name.as_deref().unwrap_or("default")
    }

    pub fn private_key(&self) -> Result<ed25519_dalek::SigningKey, Error> {
        Ok(self
            .locator
            .read_identity(self.name())?
            .key_pair(self.hd_path)?)
    }

    pub fn hd_path(&self) -> Result<u32, Error> {
        let hd_path = &self.hd_path.unwrap_or_default();
        (*hd_path)
            .try_into()
            .map_err(|_| Error::UsizeConversionError(*hd_path))
    }

    pub fn public_key(&self) -> Result<stellar_strkey::ed25519::PublicKey, Error> {
        if self.use_ledger {
            let signer: stellar_ledger::NativeSigner = (
                String::new(),
                self.hd_path.unwrap_or_default().try_into().unwrap(),
            )
                .try_into()?;
            return Ok(signer.as_ref().get_public_key_sync(self.hd_path()?)?);
        }
        if let Ok(key) = stellar_strkey::ed25519::PublicKey::from_string(self.name()) {
            Ok(key)
        } else {
            Ok(stellar_strkey::ed25519::PublicKey::from_payload(
                self.private_key()?.verifying_key().as_bytes(),
            )?)
        }
    }
}
