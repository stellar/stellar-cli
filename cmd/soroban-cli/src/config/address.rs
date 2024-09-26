use std::str::FromStr;

use crate::xdr;

use super::locator;

/// Address can be either a public key or eventually an alias of a address.
#[derive(Clone, Debug)]
pub enum Address {
    MuxedAccount(xdr::MuxedAccount),
    AliasOrSecret(String),
}

impl Default for Address {
    fn default() -> Self {
        Address::AliasOrSecret(String::default())
    }
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Strkey(#[from] stellar_strkey::DecodeError),
    #[error("Only Ed25519 and MuxedEd25519 addresses or aliases are supported")]
    InvalidAddress,
}

impl FromStr for Address {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(xdr::MuxedAccount::from_str(value).map_or_else(
            |_| Address::AliasOrSecret(value.to_string()),
            Address::MuxedAccount,
        ))
    }
}

impl Address {
    pub fn resolve(
        &self,
        locator: &locator::Args,
        hd_path: Option<usize>,
    ) -> Result<xdr::MuxedAccount, locator::Error> {
        match self {
            Address::MuxedAccount(muxed_account) => Ok(muxed_account.clone()),
            Address::AliasOrSecret(alias) => Ok(xdr::MuxedAccount::Ed25519(
                locator.read_identity(alias)?.public_key(hd_path)?.0.into(),
            )),
        }
    }
}
