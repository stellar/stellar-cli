use crate::commands::config::secret::Secret;

use super::super::{locator, secret};
use clap::arg;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Config(#[from] locator::Error),

    #[error(transparent)]
    Secret(#[from] secret::Error),

    #[error(transparent)]
    StrKey(#[from] stellar_strkey::DecodeError),
}

#[derive(Debug, clap::Parser, Clone)]
#[group(skip)]
pub struct Cmd {
    /// Name of identity to lookup, default test identity used if not provided
    pub name: Option<String>,

    /// If identity is a seed phrase use this hd path, default is 0
    #[arg(long)]
    pub hd_path: Option<usize>,

    #[command(flatten)]
    pub locator: locator::Args,
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        println!("{}", self.public_key()?.to_string());
        Ok(())
    }

    pub fn public_key(&self) -> Result<stellar_strkey::ed25519::PublicKey, Error> {
        let res = if let Some(name) = &self.name {
            self.locator.read_identity(name)?
        } else {
            Secret::test_seed_phrase()?
        };
        let key = res.key_pair(self.hd_path)?;
        Ok(stellar_strkey::ed25519::PublicKey::from_payload(
            key.public.as_bytes(),
        )?)
    }
}
