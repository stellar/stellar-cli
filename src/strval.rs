use std::{error::Error, fmt::Display};

use stellar_contract_env_host::{
    xdr::{ScObject, ScVal},
    Host,
};

#[derive(Debug)]
pub enum StrValError {
    UnknownError,
    UnknownType,
    InvalidNumberOfParts,
    InvalidValue,
}

impl Error for StrValError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}

impl Display for StrValError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "parse error: ")?;
        match self {
            Self::UnknownError => write!(f, "an unknown error occurred")?,
            Self::UnknownType => write!(f, "unknown type specified")?,
            Self::InvalidNumberOfParts => {
                write!(f, "wrong number of parts must be 2 separated by colon (:)")?;
            }
            Self::InvalidValue => write!(f, "value is not parseable to type")?,
        };
        Ok(())
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
        "i32" => ScVal::I32(val.parse()?),
        "u32" => ScVal::U32(val.parse()?),
        "i64" => {
            let v: i64 = val.parse()?;
            if let Ok(v) = v.try_into() {
                ScVal::U63(v)
            } else {
                ScVal::Object(Some(Box::new(ScObject::I64(val.parse()?))))
            }
        }
        "u64" => ScVal::Object(Some(Box::new(ScObject::U64(val.parse()?)))),
        _ => return Err(StrValError::UnknownType),
    };
    Ok(val)
}

pub fn to_string(_h: &Host, v: ScVal) -> String {
    #[allow(clippy::match_same_arms)]
    match v {
        ScVal::I32(v) => format!("i32:{}", v),
        ScVal::U32(v) => format!("u32:{}", v),
        ScVal::U63(v) => format!("i64:{}", v),
        ScVal::Static(_) => todo!(),
        ScVal::Symbol(_) => todo!(),
        ScVal::Bitset(_) => todo!(),
        ScVal::Status(_) => todo!(),
        ScVal::Object(None) => panic!(""),
        ScVal::Object(Some(b)) => match *b {
            ScObject::Box(_) => todo!(),
            ScObject::Vec(_) => todo!(),
            ScObject::Map(_) => todo!(),
            ScObject::U64(v) => format!("u64:{}", v),
            ScObject::I64(v) => format!("i64:{}", v),
            ScObject::String(_) => todo!(),
            ScObject::Binary(_) => todo!(),
            ScObject::Bigint(_) => todo!(),
            ScObject::Bigrat(_) => todo!(),
            ScObject::Ledgerkey(_) => todo!(),
            ScObject::Operation(_) => todo!(),
            ScObject::OperationResult(_) => todo!(),
            ScObject::Transaction(_) => todo!(),
            ScObject::Asset(_) => todo!(),
            ScObject::Price(_) => todo!(),
            ScObject::Accountid(_) => todo!(),
        },
    }
}
