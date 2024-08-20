use std::str::FromStr;

use crate::xdr;

#[derive(Clone, Debug)]
pub struct String64(pub xdr::String64);

impl FromStr for String64 {
    type Err = xdr::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self(xdr::String64(xdr::StringM::<64>::from_str(value)?)))
    }
}

impl From<String64> for xdr::String64 {
    fn from(builder: String64) -> Self {
        builder.0
    }
}

#[derive(Clone, Debug)]
pub struct String32(pub xdr::String32);

impl FromStr for String32 {
    type Err = xdr::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self(xdr::String32(xdr::StringM::<32>::from_str(value)?)))
    }
}

impl From<String32> for xdr::String32 {
    fn from(builder: String32) -> Self {
        builder.0
    }
}
