use std::str::FromStr;

use crate::tx::builder;

/// Address can be either a public key or eventually an alias of a address.
#[derive(Clone, Debug, Copy)]
pub enum Address {
    Ed25519(stellar_strkey::ed25519::PublicKey),
    MuxedEd25519(stellar_strkey::ed25519::MuxedAccount),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Strkey(#[from] stellar_strkey::DecodeError),
}

impl FromStr for Address {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Address::Ed25519(
            stellar_strkey::ed25519::PublicKey::from_str(value)?,
        ))
    }
}

impl From<Address> for builder::MuxedAccount {
    fn from(address: Address) -> Self {
        match address {
            Address::Ed25519(key) => key.into(),
            Address::MuxedEd25519(muxed_account) => muxed_account.into(),
        }
    }
}
impl From<&Address> for builder::MuxedAccount {
    fn from(address: &Address) -> Self {
        match address {
            Address::Ed25519(key) => key.into(),
            Address::MuxedEd25519(muxed_account) => muxed_account.into(),
        }
    }
}
