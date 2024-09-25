use std::str::FromStr;

use crate::xdr;

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
    #[error("Only Ed25519 and MuxedEd25519 addresses are supported")]
    InvalidAddress,
}

impl FromStr for Address {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match stellar_strkey::Strkey::from_string(value)? {
            stellar_strkey::Strkey::PublicKeyEd25519(public_key) => {
                Ok(Address::Ed25519(public_key))
            }
            stellar_strkey::Strkey::MuxedAccountEd25519(muxed_account) => {
                Ok(Address::MuxedEd25519(muxed_account))
            }
            _ => Err(Error::InvalidAddress),
        }
    }
}

impl From<Address> for xdr::MuxedAccount {
    fn from(address: Address) -> Self {
        match address {
            Address::Ed25519(key) => xdr::MuxedAccount::Ed25519(key.0.into()),
            Address::MuxedEd25519(stellar_strkey::ed25519::MuxedAccount { ed25519, id }) => {
                xdr::MuxedAccount::MuxedEd25519(xdr::MuxedAccountMed25519 {
                    id,
                    ed25519: ed25519.into(),
                })
            }
        }
    }
}
impl From<&Address> for xdr::MuxedAccount {
    fn from(address: &Address) -> Self {
        (*address).into()
    }
}
