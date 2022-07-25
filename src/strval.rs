use std::{error::Error, fmt::Display};
use serde_json::Value;

use stellar_contract_env_host::{
    xdr::{
        Error as XDRError,
        ScBigInt,
        ScMap,
        ScMapEntry,
        ScObject,
        ScSpecTypeDef,
        ScSpecTypeMap,
        ScSpecTypeOption,
        ScSpecTypeTuple,
        ScSpecTypeVec,
        ScStatic,
        ScVal,
        ScVec,
        VecM,
    },
    Host,
};

#[derive(Debug)]
pub enum StrValError {
    UnknownError,
    UnknownType,
    InvalidValue,
    XDR(XDRError),
    Serde(serde_json::Error),
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
            Self::Serde(_) => write!(f, "json error")?,
            Self::XDR(_) => write!(f, "xdr error")?,
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

pub fn from_string(s: &str, t: &ScSpecTypeDef) -> Result<ScVal, StrValError> {
    let val: ScVal = match t {
        // These ones have special processing when they're the top-level args. This is so we don't
        // need extra quotes around string args.
        ScSpecTypeDef::Symbol => ScVal::Symbol(s.as_bytes().try_into().map_err(|_| StrValError::InvalidValue)?),

        // This might either be a json array of u8s, or just the raw utf-8 bytes
        ScSpecTypeDef::Binary => {
            match serde_json::from_str(s) {
                // Firat, see if it is a json array
                Ok(Value::Array(raw)) => from_json(&Value::Array(raw), t)?,
                // Not a json array, just grab the bytes.
                _ => ScVal::Object(Some(ScObject::Binary(s.as_bytes().try_into().map_err(|_| StrValError::InvalidValue)?))),
            }
        }

        // For all others we just use the json parser
        _ => serde_json::from_str(s).map_err(StrValError::Serde).and_then(|raw| from_json(&raw, t))?,

    };
    Ok(val)
}

pub fn from_json(v: &Value, t: &ScSpecTypeDef) -> Result<ScVal, StrValError> {
    let val: ScVal = match (t, v) {
        // Boolean parsing
        (ScSpecTypeDef::Bool, Value::Bool(true)) =>
            ScVal::Static(ScStatic::True),
        (ScSpecTypeDef::Bool, Value::Bool(false)) =>
            ScVal::Static(ScStatic::False),

        // Vec parsing
        (ScSpecTypeDef::Vec(elem), Value::Array(raw)) => {
            let ScSpecTypeVec{ element_type } = *elem.to_owned();
            let parsed: Result<Vec<ScVal>, StrValError> = raw.iter().map(|item| -> Result<ScVal, StrValError> {
                from_json(item, &element_type)
            }).collect();
            let converted : ScVec = parsed?.try_into().map_err(StrValError::XDR)?;
            ScVal::Object(Some(ScObject::Vec(converted)))
        },

        // Number parsing
        (ScSpecTypeDef::BigInt, Value::String(s)) => {
            // TODO: This is a bit of a hack. It may not actually handle numbers bigger than
            // whatever json serde supports parsing as a "real number".
            from_string(s, &ScSpecTypeDef::BigInt)?
        },
        (ScSpecTypeDef::BigInt, Value::Number(n)) => {
            if let Some(u) = n.as_u64() {
                ScVal::Object(Some(ScObject::BigInt(
                    if u == 0 {
                        ScBigInt::Zero
                    } else {
                        let bytes: VecM<u8, 256000_u32> = u.to_be_bytes().to_vec().try_into().map_err(|_| StrValError::InvalidValue)?;
                        ScBigInt::Positive(bytes)
                    }
                )))
            } else if let Some(i) = n.as_i64() {
                ScVal::Object(Some(ScObject::BigInt(
                    if i == 0 {
                        ScBigInt::Zero
                    } else if i < 0 {
                        let bytes: VecM<u8, 256000_u32> = i.to_be_bytes().to_vec().try_into().map_err(|_| StrValError::InvalidValue)?;
                        ScBigInt::Negative(bytes)
                    } else {
                        let bytes: VecM<u8, 256000_u32> = i.to_be_bytes().to_vec().try_into().map_err(|_| StrValError::InvalidValue)?;
                        ScBigInt::Positive(bytes)
                    }
                )))
            } else  {
                return Err(StrValError::InvalidValue);
            }
        },
        (ScSpecTypeDef::I32, Value::Number(n)) =>
            {
            ScVal::I32(
                n.as_i64().
                    ok_or(StrValError::InvalidValue)?.
                    try_into().
                    map_err(|_| StrValError::InvalidValue)?
            )
        },
        (ScSpecTypeDef::I64, Value::Number(n)) =>
            ScVal::Object(Some(ScObject::I64(n.as_i64().ok_or(StrValError::InvalidValue)?))),
        (ScSpecTypeDef::U32, Value::Number(n)) => {
            ScVal::U32(
                n.as_u64().
                    ok_or(StrValError::InvalidValue)?.
                    try_into().
                    map_err(|_| StrValError::InvalidValue)?
            )
        },
        (ScSpecTypeDef::U64, Value::Number(n)) =>
            ScVal::U63(n.as_i64().ok_or(StrValError::InvalidValue)?),

        // Map parsing
        (ScSpecTypeDef::Map(map), Value::Object(raw)) => {
            let ScSpecTypeMap{key_type, value_type} = *map.to_owned();
            // TODO: What do we do if the expected key_type is not a string or symbol?
            let parsed: Result<Vec<ScMapEntry>, StrValError> = raw.iter().map(|(k, v)| -> Result<ScMapEntry, StrValError> {
                let key = from_string(k, &key_type)?;
                let val = from_json(v, &value_type)?;
                Ok(ScMapEntry{key, val})
            }).collect();
            let converted : ScMap = parsed?.try_into().map_err(StrValError::XDR)?;
            ScVal::Object(Some(ScObject::Map(converted)))
        },

        // Symbol parsing
        (ScSpecTypeDef::Symbol, Value::String(s)) =>
            ScVal::Symbol(s.as_bytes().try_into().map_err(|_| StrValError::InvalidValue)?),

        // Binary parsing
        (ScSpecTypeDef::Binary, Value::String(s)) =>
            ScVal::Object(Some(ScObject::Binary(s.as_bytes().try_into().map_err(|_| StrValError::InvalidValue)?))),
        (ScSpecTypeDef::Binary, Value::Array(raw)) => {
            let b: Result<Vec<u8>, StrValError> = raw.iter().map(|item| {
                item.as_u64().
                    ok_or(StrValError::InvalidValue)?.
                    try_into().
                    map_err(|_| StrValError::InvalidValue)
            }).collect();
            let converted : VecM<u8, 256000_u32> = b?.try_into().map_err(StrValError::XDR)?;
            ScVal::Object(Some(ScObject::Binary(converted)))
        },

        // Option parsing
        (ScSpecTypeDef::Option(_), Value::Null) =>
            // is null -> void the right thing here?
            ScVal::Object(None),
        (ScSpecTypeDef::Option(elem), v) => {
            let ScSpecTypeOption{ value_type } = *elem.to_owned();
            ScVal::Object(Some(from_json(v, &value_type)?.try_into().map_err(|_| StrValError::InvalidValue)?))
        },

        // Tuple parsing
        (ScSpecTypeDef::Tuple(elem), Value::Array(raw)) => {
            let ScSpecTypeTuple{ value_types } = *elem.to_owned();
            if raw.len() != value_types.len() {
                return Err(StrValError::InvalidValue);
            };
            let parsed: Result<Vec<ScVal>, StrValError> = raw.iter().zip(value_types.iter()).map(|(item, t)| {
                from_json(item, t)
            }).collect();
            let converted : ScVec = parsed?.try_into().map_err(StrValError::XDR)?;
            ScVal::Object(Some(ScObject::Vec(converted)))
        },

        // TODO: Implement the rest of these
        // ScSpecTypeDef::Bitset => {},
        // ScSpecTypeDef::Status => {},
        // ScSpecTypeDef::Result(Box<ScSpecTypeResult>) => {},
        // ScSpecTypeDef::Set(Box<ScSpecTypeSet>) => {},
        // ScSpecTypeDef::Udt(ScSpecTypeUdt) => {},
        _ => return Err(StrValError::UnknownType),
    };
    Ok(val)
}

pub fn to_string(v: &ScVal) -> Result<String, StrValError> {
    #[allow(clippy::match_same_arms)]
    Ok(match v {
        // If symbols are a top-level thing we omit the wrapping quotes
        // TODO: Decide if this is a good idea or not.
        ScVal::Symbol(v) => format!(
            "{}",
            std::str::from_utf8(v.as_slice()).map_err(|_| StrValError::InvalidValue)?
        ),
        _ => serde_json::to_string(&to_json(v)?).map_err(StrValError::Serde)?,
    })
}

pub fn to_json(v: &ScVal) -> Result<Value, StrValError> {
    #[allow(clippy::match_same_arms)]
    let val: Value = match v {
        ScVal::Static(v) => match v {
            ScStatic::True => Value::Bool(true),
            ScStatic::False => Value::Bool(false),
            ScStatic::Void => Value::Null,
            _ => return Err(StrValError::InvalidValue),
        },
        ScVal::U63(v) => Value::Number(serde_json::Number::from(v.clone())),
        ScVal::U32(v) => Value::Number(serde_json::Number::from(v.clone())),
        ScVal::I32(v) => Value::Number(serde_json::Number::from(v.clone())),
        ScVal::Symbol(v) => Value::String(
            std::str::from_utf8(v.as_slice()).
                map_err(|_| StrValError::InvalidValue)?.to_string()
        ),
        ScVal::Bitset(_) => todo!(),
        ScVal::Status(_) => todo!(),
        ScVal::Object(None) => Value::Null,
        ScVal::Object(Some(b)) => match b {
            ScObject::Vec(v) => {
                let values: Result<Vec<Value>, StrValError> = v.iter().map(|item| -> Result<Value, StrValError> {
                    to_json(item)
                }).collect();
                Value::Array(values?)
            },
            ScObject::Map(v) => {
                // TODO: What do we do if the key is not a string?
                let mut m = serde_json::Map::<String, Value>::with_capacity(v.len());
                for ScMapEntry{key, val} in v.iter() {
                    let k: String = to_string(key)?;
                    let v: Value = to_json(val).map_err(|_| StrValError::InvalidValue)?;
                    m.insert(k, v);
                };
                Value::Object(m)
            },
            ScObject::U64(v) => Value::Number(serde_json::Number::from(v.clone())),
            ScObject::I64(v) => Value::Number(serde_json::Number::from(v.clone())),
            ScObject::Binary(v) => Value::Array(
                v.to_vec().iter().map(|item|
                    Value::Number(serde_json::Number::from(item.clone()))
                ).collect()
            ),
            ScObject::BigInt(n) => {
                // TODO: This is a hack. Should output as a string if the number is too big. Or a
                // byte array? Either way, this won't currently support numbers > u64. Need to
                // implement conversions/comparisons so we can tell if:
                // (n > u64::MAX || n < i64::MIN)
                Value::Number(match n {
                    ScBigInt::Zero => serde_json::Number::from(0),
                    ScBigInt::Negative(i) => {
                        let mut bytes: Vec<u8> = i.to_vec();
                        while bytes.len() < 8 { bytes.insert(0, 0) };
                        if bytes.len() == 9 { bytes.remove(0); };
                        serde_json::Number::from(i64::from_be_bytes(bytes.try_into().map_err(|_| StrValError::InvalidValue)?))
                    },
                    ScBigInt::Positive(u) => {
                        let mut bytes: Vec<u8> = u.to_vec();
                        while bytes.len() < 8 { bytes.insert(0, 0) };
                        serde_json::Number::from(u64::from_be_bytes(bytes.try_into().map_err(|_| StrValError::InvalidValue)?))
                    },
                })
            },
            ScObject::Hash(_) => todo!(),
            ScObject::PublicKey(_) => todo!(),
        },
        v => serde_json::to_value(v).map_err(StrValError::Serde)?,
    };
    Ok(val)
}
