use std::{
    fmt::{self, Display, Formatter},
    str::FromStr,
};

use crate::{
    signer::{self, ledger},
    xdr,
};

use super::{key, locator, secret};

/// Address can be either a public key or eventually an alias of a address.
#[derive(Clone, Debug)]
pub enum UnresolvedMuxedAccount {
    Resolved(xdr::MuxedAccount),
    AliasOrSecret(String),
    Ledger(u32),
}

impl Default for UnresolvedMuxedAccount {
    fn default() -> Self {
        UnresolvedMuxedAccount::AliasOrSecret(String::default())
    }
}

impl Display for UnresolvedMuxedAccount {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            UnresolvedMuxedAccount::Resolved(muxed_account) => write!(f, "{muxed_account}"),
            UnresolvedMuxedAccount::AliasOrSecret(alias_or_secret) => {
                write!(f, "{alias_or_secret}")
            }
            UnresolvedMuxedAccount::Ledger(hd_path) => write!(f, "ledger:{hd_path}"),
        }
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
    #[error(transparent)]
    Key(#[from] key::Error),
    #[error("Address cannot be used to sign {0}")]
    CannotSign(xdr::MuxedAccount),
    #[error("Ledger cannot reveal private keys")]
    LedgerPrivateKeyRevealNotSupported,
    #[error("Invalid key name: {0}\n `ledger` is not allowed")]
    LedgerIsInvalidKeyName(String),
    #[error("Invalid key name: {0}\n only alphanumeric characters, underscores (_), and hyphens (-) are allowed.")]
    InvalidKeyNameCharacters(String),
    #[error("Invalid key name: {0}\n keys cannot exceed 250 characters")]
    InvalidKeyNameLength(String),
    #[error("Invalid key name: {0}\n keys cannot be the word \"ledger\"")]
    InvalidKeyName(String),
    #[error("Ledger not supported in this context")]
    LedgerNotSupported,
    #[error(transparent)]
    Ledger(#[from] signer::ledger::Error),
    #[error("Invalid name: {0}\n only alphanumeric characters, underscores (_), and hyphens (-) are allowed.")]
    InvalidNameCharacters(String),
    #[error("Invalid name: {0}\n names cannot exceed 250 characters")]
    InvalidNameLength(String),
}

impl FromStr for UnresolvedMuxedAccount {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.starts_with("ledger") {
            if let Some(ledger) = parse_ledger(value) {
                return Ok(UnresolvedMuxedAccount::Ledger(ledger));
            }
        }
        Ok(xdr::MuxedAccount::from_str(value).map_or_else(
            |_| UnresolvedMuxedAccount::AliasOrSecret(value.to_string()),
            UnresolvedMuxedAccount::Resolved,
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

impl UnresolvedMuxedAccount {
    pub async fn resolve_muxed_account(
        &self,
        locator: &locator::Args,
        hd_path: Option<usize>,
    ) -> Result<xdr::MuxedAccount, Error> {
        match self {
            UnresolvedMuxedAccount::Ledger(hd_path) => Ok(xdr::MuxedAccount::Ed25519(
                ledger::new(*hd_path).await?.public_key().await?.0.into(),
            )),
            UnresolvedMuxedAccount::Resolved(_) | UnresolvedMuxedAccount::AliasOrSecret(_) => {
                self.resolve_muxed_account_sync(locator, hd_path)
            }
        }
    }

    pub fn resolve_muxed_account_sync(
        &self,
        locator: &locator::Args,
        hd_path: Option<usize>,
    ) -> Result<xdr::MuxedAccount, Error> {
        match self {
            UnresolvedMuxedAccount::Resolved(muxed_account) => Ok(muxed_account.clone()),
            UnresolvedMuxedAccount::AliasOrSecret(alias_or_secret) => {
                Ok(locator.read_key(alias_or_secret)?.muxed_account(hd_path)?)
            }
            UnresolvedMuxedAccount::Ledger(_) => Err(Error::LedgerNotSupported),
        }
    }

    pub fn resolve_secret(&self, locator: &locator::Args) -> Result<secret::Secret, Error> {
        match &self {
            UnresolvedMuxedAccount::Resolved(muxed_account) => {
                Err(Error::CannotSign(muxed_account.clone()))
            }
            UnresolvedMuxedAccount::AliasOrSecret(alias_or_secret) => {
                Ok(locator.read_key(alias_or_secret)?.try_into()?)
            }
            UnresolvedMuxedAccount::Ledger(_) => Err(Error::LedgerPrivateKeyRevealNotSupported),
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
            return Err(Error::InvalidKeyNameCharacters(s.to_string()));
        }
        if s == "ledger" {
            return Err(Error::InvalidKeyName(s.to_string()));
        }
        if s.len() > 250 {
            return Err(Error::InvalidKeyNameLength(s.to_string()));
        }
        Ok(KeyName(s.to_string()))
    }
}

impl Display for KeyName {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

fn allowed_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-'
}

pub fn validate_name(s: &str) -> Result<(), Error> {
    if s.is_empty() || s.len() > 250 {
        return Err(Error::InvalidNameLength(s.to_string()));
    }
    if !s.chars().all(allowed_char) {
        return Err(Error::InvalidNameCharacters(s.to_string()));
    }
    Ok(())
}

#[derive(Clone, Debug)]
pub struct NetworkName(pub String);

impl std::ops::Deref for NetworkName {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::str::FromStr for NetworkName {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        validate_name(s)?;
        Ok(NetworkName(s.to_string()))
    }
}

impl Display for NetworkName {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Debug)]
pub struct AliasName(pub String);

impl std::ops::Deref for AliasName {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::str::FromStr for AliasName {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        validate_name(s)?;
        Ok(AliasName(s.to_string()))
    }
}

impl Display for AliasName {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn network_name_valid() {
        assert!("my-network".parse::<NetworkName>().is_ok());
        assert!("my_network_123".parse::<NetworkName>().is_ok());
        assert!("ledger".parse::<NetworkName>().is_ok());
    }

    #[test]
    fn network_name_rejects_path_traversal() {
        assert!("../evil".parse::<NetworkName>().is_err());
        assert!("../../etc/passwd".parse::<NetworkName>().is_err());
        assert!("foo/bar".parse::<NetworkName>().is_err());
        assert!("foo\\bar".parse::<NetworkName>().is_err());
    }

    #[test]
    fn network_name_rejects_too_long() {
        assert!("a".repeat(251).parse::<NetworkName>().is_err());
        assert!("a".repeat(250).parse::<NetworkName>().is_ok());
    }

    #[test]
    fn alias_name_valid() {
        assert!("my_alias_123".parse::<AliasName>().is_ok());
        assert!("ledger".parse::<AliasName>().is_ok());
    }

    #[test]
    fn alias_name_rejects_path_traversal() {
        assert!("../evil".parse::<AliasName>().is_err());
        assert!("../../etc/passwd".parse::<AliasName>().is_err());
        assert!("foo/bar".parse::<AliasName>().is_err());
        assert!("foo\\bar".parse::<AliasName>().is_err());
    }

    #[test]
    fn alias_name_rejects_too_long() {
        assert!("a".repeat(251).parse::<AliasName>().is_err());
        assert!("a".repeat(250).parse::<AliasName>().is_ok());
    }

    #[test]
    fn network_name_rejects_empty() {
        assert!("".parse::<NetworkName>().is_err());
    }

    #[test]
    fn alias_name_rejects_empty() {
        assert!("".parse::<AliasName>().is_err());
    }
}
