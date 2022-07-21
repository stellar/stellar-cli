use std::{error::Error, fmt::Display};

use stellar_contract_env_host::{
    xdr::{ScObject, ScVal, ScStatic, ScSpecTypeDef},
    Host,
};

#[derive(Debug)]
pub enum StrValError {
    UnknownError,
    UnknownType,
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

// TODO: Do json parsing/serialization here. Particularly like binary byte slices, and string
// escaping.
pub fn from_string(_h: &Host, s: &str, t: &ScSpecTypeDef) -> Result<ScVal, StrValError> {
    let val: ScVal = match t {
        // SHould this be u63? or u64?
        ScSpecTypeDef::U64 => ScVal::U63(s.parse()?),
        ScSpecTypeDef::I64 => ScVal::Object(Some(ScObject::I64(s.parse()?))),
        ScSpecTypeDef::U32 => ScVal::U32(s.parse()?),
        ScSpecTypeDef::I32 => ScVal::I32(s.parse()?),
        ScSpecTypeDef::Bool => match s.to_lowercase().trim() {
            "true" => ScVal::Static(ScStatic::True),
            "false" => ScVal::Static(ScStatic::False),
            _ => return Err(StrValError::InvalidValue),
        },
        ScSpecTypeDef::Symbol => ScVal::Symbol(s.as_bytes().try_into().map_err(|_| StrValError::InvalidValue)?),
        // ScSpecTypeDef::Bitset => {},
        // ScSpecTypeDef::Status => {},
        ScSpecTypeDef::Binary => ScVal::Object(Some(ScObject::Binary(s.as_bytes().try_into().map_err(|_| StrValError::InvalidValue)?))),
        // ScSpecTypeDef::BigInt => ScVal::Object(Some(ScObject::BigInt(s.parse()?))),
        // ScSpecTypeDef::Option(Box<ScSpecTypeOption>) => {},
        // ScSpecTypeDef::Result(Box<ScSpecTypeResult>) => {},
        // ScSpecTypeDef::Vec(Box<ScSpecTypeVec>) => {},
        // ScSpecTypeDef::Map(Box<ScSpecTypeMap>) => {},
        // ScSpecTypeDef::Set(Box<ScSpecTypeSet>) => {},
        // ScSpecTypeDef::Tuple(Box<ScSpecTypeTuple>) => {},
        // ScSpecTypeDef::Udt(ScSpecTypeUdt) => {},
        _ => return Err(StrValError::UnknownType),

    };
    Ok(val)
}

pub fn to_string(_h: &Host, v: ScVal) -> String {
    #[allow(clippy::match_same_arms)]
    match v {
        ScVal::I32(v) => format!("{}", v),
        ScVal::U32(v) => format!("{}", v),
        ScVal::U63(v) => format!("{}", v),
        ScVal::Static(v) => match v {
            ScStatic::True => "true",
            ScStatic::False => "false",
            ScStatic::Void => "void",
            _ => "todo!"
        }.to_string(),
        ScVal::Symbol(v) => format!(
            "{}",
            std::str::from_utf8(v.as_slice()).expect("non-UTF-8 in symbol")
        ),
        ScVal::Bitset(_) => todo!(),
        ScVal::Status(_) => todo!(),
        ScVal::Object(None) => panic!(""),
        ScVal::Object(Some(b)) => match b {
            ScObject::Vec(_) => todo!(),
            ScObject::Map(_) => todo!(),
            ScObject::U64(v) => format!("{}", v),
            ScObject::I64(v) => format!("{}", v),
            ScObject::Binary(_) => todo!(),
            ScObject::BigInt(_) => todo!(),
            ScObject::Hash(_) => todo!(),
            ScObject::PublicKey(_) => todo!(),
        },
    }
}
