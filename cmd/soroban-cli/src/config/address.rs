use std::{
    fmt::{self, Display, Formatter},
    str::FromStr,
};

use crate::{signer, xdr};

use super::{key, locator, secret, utils};

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

impl Display for UnresolvedMuxedAccount {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            UnresolvedMuxedAccount::Resolved(muxed_account) => write!(f, "{muxed_account}"),
            UnresolvedMuxedAccount::AliasOrSecret(alias_or_secret) => {
                write!(f, "{alias_or_secret}")
            }
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
    #[error("Invalid key name: {0}\n only alphanumeric characters, underscores (_), and hyphens (-) are allowed.")]
    InvalidKeyNameCharacters(String),
    #[error("Invalid key name: {0}\n keys cannot exceed 250 characters")]
    InvalidKeyNameLength(String),
    #[error(transparent)]
    Name(#[from] utils::Error),
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
            UnresolvedMuxedAccount::AliasOrSecret(alias_or_secret) => Ok(locator
                .read_key_with_secure_store_cache(alias_or_secret, hd_path)?
                .muxed_account(hd_path)?),
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
        utils::validate_name(s).map_err(|e| match e {
            utils::Error::InvalidNameLength(s) => Error::InvalidKeyNameLength(s),
            utils::Error::InvalidNameCharacters(s) => Error::InvalidKeyNameCharacters(s),
        })?;
        Ok(KeyName(s.to_string()))
    }
}

impl Display for KeyName {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub fn validate_name(s: &str) -> Result<(), Error> {
    Ok(utils::validate_name(s)?)
}

#[derive(Clone, Debug)]
pub struct NetworkName(String);

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
pub struct AliasName(String);

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

#[derive(Clone, Debug)]
pub struct ContractName(String);

impl std::ops::Deref for ContractName {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::str::FromStr for ContractName {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        validate_name(s)?;
        Ok(ContractName(s.to_string()))
    }
}

impl Display for ContractName {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<std::path::Path> for ContractName {
    fn as_ref(&self) -> &std::path::Path {
        std::path::Path::new(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ledger_shorthand_is_not_recognized() {
        match "ledger".parse::<UnresolvedMuxedAccount>().unwrap() {
            UnresolvedMuxedAccount::AliasOrSecret(s) => assert_eq!(s, "ledger"),
            UnresolvedMuxedAccount::Resolved(m) => panic!("unexpected resolved muxed: {m}"),
        }
    }

    #[test]
    fn ledger_indexed_shorthand_is_not_recognized() {
        match "ledger:5".parse::<UnresolvedMuxedAccount>().unwrap() {
            UnresolvedMuxedAccount::AliasOrSecret(s) => assert_eq!(s, "ledger:5"),
            UnresolvedMuxedAccount::Resolved(m) => panic!("unexpected resolved muxed: {m}"),
        }
    }

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

    #[test]
    fn contract_name_valid() {
        assert!("hello-world".parse::<ContractName>().is_ok());
        assert!("my_contract_123".parse::<ContractName>().is_ok());
    }

    #[test]
    fn contract_name_rejects_path_traversal() {
        assert!("../evil".parse::<ContractName>().is_err());
        assert!("../../etc/passwd".parse::<ContractName>().is_err());
        assert!("foo/bar".parse::<ContractName>().is_err());
        assert!("foo\\bar".parse::<ContractName>().is_err());
    }

    #[test]
    fn contract_name_rejects_too_long() {
        assert!("a".repeat(251).parse::<ContractName>().is_err());
        assert!("a".repeat(250).parse::<ContractName>().is_ok());
    }

    #[test]
    fn contract_name_rejects_empty() {
        assert!("".parse::<ContractName>().is_err());
    }
}
