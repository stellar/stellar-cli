use clap::arg;

use crate::{
    commands::config::{address, locator, secret},
    xdr,
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Config(#[from] locator::Error),

    #[error(transparent)]
    Secret(#[from] secret::Error),

    #[error(transparent)]
    StrKey(#[from] stellar_strkey::DecodeError),

    #[error(transparent)]
    Address(#[from] address::Error),
}

#[derive(Debug, clap::Parser, Clone)]
#[group(skip)]
pub struct Cmd {
    /// Name of identity to lookup, ledger, or secret key
    pub name: address::Address,

    /// If identity is a seed phrase use this hd path, default is 0
    #[arg(long)]
    pub hd_path: Option<usize>,

    #[command(flatten)]
    pub locator: locator::Args,
}

impl Cmd {
    pub async fn run(&self) -> Result<(), Error> {
        println!("{}", self.public_key().await?);
        Ok(())
    }

    pub async fn public_key(&self) -> Result<stellar_strkey::ed25519::PublicKey, Error> {
        match self
            .name
            .resolve_muxed_account(&self.locator, self.hd_path)
            .await?
        {
            xdr::MuxedAccount::Ed25519(pk) => Ok(stellar_strkey::ed25519::PublicKey(pk.0)),
            xdr::MuxedAccount::MuxedEd25519(xdr::MuxedAccountMed25519 { ed25519, .. }) => {
                Ok(stellar_strkey::ed25519::PublicKey(ed25519.0))
            }
        }
    }
}
