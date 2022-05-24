use std::{error::Error, fmt::Display};

use stellar_contract_env_host::{xdr::ScVal, Host};

#[derive(Debug)]
pub enum StrValError {
    UnknownError,
    UnknownType,
    InvalidNumberOfParts,
    InvalidValue,
}

impl Error for StrValError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::UnknownError => None,
            Self::UnknownType => None,
            Self::InvalidNumberOfParts => None,
            Self::InvalidValue => None,
        }
    }
}

impl Display for StrValError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "parse error: ")?;
        Ok(match self {
            Self::UnknownError => write!(f, "an unknown error occurred")?,
            Self::UnknownType => write!(f, "unknown type specified")?,
            Self::InvalidNumberOfParts => {
                write!(f, "wrong number of parts must be 2 separated by colon (:)")?
            }
            Self::InvalidValue => write!(f, "value is not parseable to type")?,
        })
    }
}

impl From<std::num::ParseIntError> for StrValError {
    fn from(_: std::num::ParseIntError) -> Self {
        StrValError::InvalidValue
    }
}

impl From<()> for StrValError {
    fn from(_: ()) -> Self {
        StrValError::UnknownError
    }
}

pub fn from_string(_h: &Host, s: &str) -> Result<ScVal, StrValError> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 2 {
        return Err(StrValError::InvalidNumberOfParts);
    }
    let typ = parts[0];
    let val = parts[1];
    let val: ScVal = match typ {
        "i32" => ScVal::I32(val.parse::<i32>()?),
        "u32" => ScVal::U32(val.parse::<u32>()?),
        _ => return Err(StrValError::UnknownType),
    };
    Ok(val)
}

pub fn to_string(_h: &Host, v: ScVal) -> Result<String, StrValError> {
    let s = match v {
        ScVal::I32(v) => format!("i32:{}", v),
        ScVal::U32(v) => format!("u32:{}", v),
        _ => return Err(StrValError::UnknownType),
    };
    Ok(s)
}
