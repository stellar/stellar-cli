use std::fmt::{Display, Formatter};

use soroban_sdk::xdr::{self, Limits, WriteXdr};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Expect xdr string")]
    XdrStringExpected,
    #[error("Expect result")]
    ResultExpected,
}

pub enum TxnResult<T> {
    Xdr(String),
    Res(T),
}

impl<T> TxnResult<T> {
    pub fn from_xdr(res: &impl WriteXdr) -> Result<Self, xdr::Error> {
        Ok(TxnResult::Xdr(res.to_xdr_base64(Limits::none())?))
    }

    pub fn xdr(&self) -> Option<&str> {
        match self {
            TxnResult::Xdr(xdr) => Some(xdr),
            TxnResult::Res(_) => None,
        }
    }

    pub fn res(&self) -> Option<&T> {
        match self {
            TxnResult::Res(res) => Some(res),
            TxnResult::Xdr(_) => None,
        }
    }

    pub fn into_res(self) -> Option<T> {
        match self {
            TxnResult::Res(res) => Some(res),
            TxnResult::Xdr(_) => None,
        }
    }

    pub fn try_xdr(&self) -> Result<&str, Error> {
        self.xdr().ok_or(Error::XdrStringExpected)
    }

    pub fn try_res(&self) -> Result<&T, Error> {
        self.res().ok_or(Error::ResultExpected)
    }
    pub fn try_into_res(self) -> Result<T, Error> {
        match self {
            TxnResult::Res(res) => Ok(res),
            TxnResult::Xdr(_) => Err(Error::XdrStringExpected),
        }
    }
}

impl<T> Display for TxnResult<T>
where
    T: Display,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            TxnResult::Xdr(xdr) => write!(f, "{xdr}"),
            TxnResult::Res(res) => write!(f, "{res}"),
        }
    }
}
