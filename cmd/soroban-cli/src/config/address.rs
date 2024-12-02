use std::str::FromStr;

use crate::xdr;

use super::{locator, secret};

/// Address can be either a public key or eventually an alias of a address.
#[derive(Clone, Debug)]
pub enum UnresolvedMuxedAccount {
    Resolved(xdr::MuxedAccount),
    AliasOrSecret(String),
}

impl Default for UnresolvedMuxedAccount {
    fn default() -> Self {
        UnresolvedMuxedAccount::AliasOrSecret(String::default())
    }
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Locator(#[from] locator::Error),
    #[error(transparent)]
    Secret(#[from] secret::Error),
    #[error("Address cannot be used to sign {0}")]
    CannotSign(xdr::MuxedAccount),
}

impl FromStr for UnresolvedMuxedAccount {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(xdr::MuxedAccount::from_str(value).map_or_else(
            |_| UnresolvedMuxedAccount::AliasOrSecret(value.to_string()),
            UnresolvedMuxedAccount::Resolved,
        ))
    }
}

impl UnresolvedMuxedAccount {
    pub fn resolve_muxed_account(
        &self,
        locator: &locator::Args,
        hd_path: Option<usize>,
    ) -> Result<xdr::MuxedAccount, Error> {
        match self {
            UnresolvedMuxedAccount::Resolved(muxed_account) => Ok(muxed_account.clone()),
            UnresolvedMuxedAccount::AliasOrSecret(alias) => {
                Self::resolve_muxed_account_with_alias(alias, locator, hd_path)
            }
        }
    }

    pub fn resolve_muxed_account_with_alias(
        alias: &str,
        locator: &locator::Args,
        hd_path: Option<usize>,
    ) -> Result<xdr::MuxedAccount, Error> {
        alias.parse().or_else(|_| {
            Ok(xdr::MuxedAccount::Ed25519(
                locator.read_identity(alias)?.public_key(hd_path)?.0.into(),
            ))
        })
    }

    pub fn resolve_secret(&self, locator: &locator::Args) -> Result<secret::Secret, Error> {
        match &self {
            UnresolvedMuxedAccount::Resolved(muxed_account) => Err(Error::CannotSign(muxed_account.clone())),
            UnresolvedMuxedAccount::AliasOrSecret(alias) => Ok(locator.read_identity(alias)?),
        }
    }
}
