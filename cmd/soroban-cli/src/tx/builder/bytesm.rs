use std::str::FromStr;

use crate::xdr;

#[derive(Clone, Debug)]
pub struct Bytes64(pub xdr::BytesM<64>);

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Xdr(#[from] xdr::Error),
    #[error(transparent)]
    Hex(#[from] hex::FromHexError),
}

impl FromStr for Bytes64 {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self(xdr::BytesM::<64>::try_from(&hex::decode(value)?)?))
    }
}

impl From<Bytes64> for xdr::BytesM<64> {
    fn from(builder: Bytes64) -> Self {
        builder.0
    }
}
