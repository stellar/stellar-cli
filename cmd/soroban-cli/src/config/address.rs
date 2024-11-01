use std::str::FromStr;

use crate::{
    signer::{self, native_ledger},
    xdr,
};

use super::{locator, secret};

/// Address can be either a public key or eventually an alias of a address.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Address {
    MuxedAccount(xdr::MuxedAccount),
    AliasOrSecret(String),
    Ledger(u32),
}

impl Default for Address {
    fn default() -> Self {
        Address::AliasOrSecret(String::default())
    }
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Locator(#[from] locator::Error),
    #[error(transparent)]
    Secret(#[from] secret::Error),
    #[error(transparent)]
    Signer(#[from] signer::Error),
    #[error("Address cannot be used to sign {0}")]
    CannotSign(xdr::MuxedAccount),
    #[error("Ledger not supported")]
    LedgerNotSupported,
    #[error("Invalid key name: {0}\n only alphanumeric characters, `_`and `-` are allowed")]
    InvalidKeyName(String),
    #[error("Invalid key name: {0}\n `ledger` is not allowed")]
    LedgerIsInvalidKeyName(String),
}

impl FromStr for Address {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.starts_with("ledger") {
            if let Some(ledger) = parse_ledger(value) {
                return Ok(Address::Ledger(ledger));
            }
        }
        Ok(xdr::MuxedAccount::from_str(value).map_or_else(
            |_| Address::AliasOrSecret(value.to_string()),
            Address::MuxedAccount,
        ))
    }
}

fn parse_ledger(value: &str) -> Option<u32> {
    let vals: Vec<_> = value.split(':').collect();
    if vals.len() > 2 {
        return None;
    }
    if vals.len() == 1 {
        return Some(0);
    }
    vals[1].parse().ok()
}

impl Address {
    pub async fn resolve_muxed_account(
        &self,
        locator: &locator::Args,
        hd_path: Option<usize>,
    ) -> Result<xdr::MuxedAccount, Error> {
        match self {
            Address::MuxedAccount(muxed_account) => Ok(muxed_account.clone()),
            Address::AliasOrSecret(alias) => alias.parse().or_else(|_| {
                Ok(xdr::MuxedAccount::Ed25519(
                    locator.read_identity(alias)?.public_key(hd_path)?.0.into(),
                ))
            }),
            Address::Ledger(hd_path) => Ok(xdr::MuxedAccount::Ed25519(
                native_ledger(*hd_path)?.public_key().await?.0.into(),
            )),
        }
    }

    pub fn resolve_muxed_account_sync(
        &self,
        locator: &locator::Args,
        hd_path: Option<usize>,
    ) -> Result<xdr::MuxedAccount, Error> {
        match self {
            Address::MuxedAccount(muxed_account) => Ok(muxed_account.clone()),
            Address::AliasOrSecret(alias) => alias.parse().or_else(|_| {
                Ok(xdr::MuxedAccount::Ed25519(
                    locator.read_identity(alias)?.public_key(hd_path)?.0.into(),
                ))
            }),
            Address::Ledger(_) => Err(Error::LedgerNotSupported),
        }
    }

    pub fn resolve_secret(&self, locator: &locator::Args) -> Result<secret::Secret, Error> {
        match &self {
            Address::AliasOrSecret(alias) => Ok(locator.read_identity(alias)?),
            a => Err(Error::CannotSign(
                a.resolve_muxed_account_sync(locator, None)?,
            )),
        }
    }
}

#[derive(Clone, Debug)]
pub struct KeyName(pub String);

impl std::ops::Deref for KeyName {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::str::FromStr for KeyName {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if !s.chars().all(allowed_char) {
            return Err(Error::InvalidKeyName(s.to_string()));
        }
        if s == "ledger" {
            return Err(Error::InvalidKeyName(s.to_string()));
        }
        Ok(KeyName(s.to_string()))
    }
}

fn allowed_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ledger_address() {
        let address = Address::from_str("ledger:0").unwrap();
        assert_eq!(address, Address::Ledger(0));
        let address = Address::from_str("ledger:1").unwrap();
        assert_eq!(address, Address::Ledger(1));
        let address = Address::from_str("ledger").unwrap();
        assert_eq!(address, Address::Ledger(0));
    }

    #[test]
    fn invalid_ledger_address() {
        assert_eq!(
            Address::AliasOrSecret("ledger:".to_string()),
            Address::from_str("ledger:").unwrap()
        );
        assert_eq!(
            Address::AliasOrSecret("ledger:1:2".to_string()),
            Address::from_str("ledger:1:2").unwrap()
        );
    }
}
