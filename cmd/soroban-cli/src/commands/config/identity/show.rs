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

#[derive(Debug, clap::Parser)]
pub struct Cmd {
    /// Name of identity to lookup
    pub name: String,

    /// If identity is a seed phrase use this hd path, default is 0
    #[arg(long)]
    pub hd_path: Option<usize>,

    #[clap(skip)]
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
