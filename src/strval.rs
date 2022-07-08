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
            if v >= 0 {
                ScVal::U63(v)
            } else {
                ScVal::Object(Some(ScObject::I64(v)))
            }
        }
        "u64" => ScVal::Object(Some(ScObject::U64(val.parse()?))),
        "sym" => ScVal::Symbol(
            val.as_bytes()
                .try_into()
                .map_err(|_| StrValError::InvalidValue)?,
        ),
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
        ScVal::Static(_) => "todo!".to_string(), //TODO:fix this
        ScVal::Symbol(v) => format!(
            "sym:{}",
            std::str::from_utf8(v.as_slice()).expect("non-UTF-8 in symbol")
        ),
        ScVal::Bitset(_) => todo!(),
        ScVal::Status(_) => todo!(),
        ScVal::Object(None) => panic!(""),
        ScVal::Object(Some(b)) => match b {
            ScObject::Vec(_) => todo!(),
            ScObject::Map(_) => todo!(),
            ScObject::U64(v) => format!("u64:{}", v),
            ScObject::I64(v) => format!("i64:{}", v),
            ScObject::Binary(_) => todo!(),
            ScObject::BigInt(_) => todo!(),
            ScObject::Hash(_) => todo!(),
            ScObject::PublicKey(_) => todo!(),
        },
    }
}
