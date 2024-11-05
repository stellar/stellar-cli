use clap::arg;

use crate::config::{key, locator};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Config(#[from] locator::Error),

    #[error(transparent)]
    Key(#[from] key::Error),
}

#[derive(Debug, clap::Parser, Clone)]
#[group(skip)]
pub struct Cmd {
    /// Name of identity to lookup, default is test identity
    pub name: String,

    /// If identity is a seed phrase use this hd path, default is 0
    #[arg(long)]
    pub hd_path: Option<usize>,

    #[command(flatten)]
    pub locator: locator::Args,
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        println!("{}", self.private_key()?.to_string());
        Ok(())
    }

    pub fn private_key(&self) -> Result<stellar_strkey::ed25519::PrivateKey, Error> {
        Ok(self
            .locator
            .read_identity(&self.name)?
            .private_key(self.hd_path)?)
    }
}
