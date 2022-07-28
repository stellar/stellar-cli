use serde_json::Value;
use std::{error::Error, fmt::Display, str::FromStr};

use num_bigint::{BigInt, Sign};
use soroban_env_host::xdr::{
    Error as XdrError, ScBigInt, ScMap, ScMapEntry, ScObject, ScSpecTypeDef, ScSpecTypeMap,
    ScSpecTypeOption, ScSpecTypeTuple, ScSpecTypeVec, ScStatic, ScVal, ScVec, VecM,
};

#[derive(Debug)]
pub enum StrValError {
    UnknownError,
    InvalidValue,
    Xdr(XdrError),
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
            Self::InvalidValue => write!(f, "value is not parseable to type")?,
            Self::Serde(e) => write!(f, "{}", e)?,
            Self::Xdr(e) => write!(f, "{}", e)?,
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
        ScSpecTypeDef::Symbol => ScVal::Symbol(
            s.as_bytes()
                .try_into()
                .map_err(|_| StrValError::InvalidValue)?,
        ),

        // This might either be a json array of u8s, or just the raw utf-8 bytes
        ScSpecTypeDef::Binary => {
            match serde_json::from_str(s) {
                // First, see if it is a json array
                Ok(Value::Array(raw)) => from_json(&Value::Array(raw), t)?,
                // Not a json array, just grab the bytes.
                _ => ScVal::Object(Some(ScObject::Binary(
                    s.as_bytes()
                        .try_into()
                        .map_err(|_| StrValError::InvalidValue)?,
                ))),
            }
        }

        // Might have wrapping quotes if it is negative. e.g. "-5"
        ScSpecTypeDef::BigInt => {
            if let Ok(Value::String(raw)) = serde_json::from_str(s) {
                // First, see if it is a json string, strip the quotes and recurse
                from_string(&raw, &ScSpecTypeDef::BigInt)?
            } else {
                let big = BigInt::from_str(s).map_err(|_| StrValError::InvalidValue)?;
                let (sign, bytes) = big.to_bytes_be();
                let b: VecM<u8, 256_000_u32> = bytes.try_into().map_err(StrValError::Xdr)?;
                ScVal::Object(Some(ScObject::BigInt(match sign {
                    Sign::NoSign => ScBigInt::Zero,
                    Sign::Minus => ScBigInt::Negative(b),
                    Sign::Plus => ScBigInt::Positive(b),
                })))
            }
        }

        // For all others we just use the json parser
        _ => serde_json::from_str(s)
            .map_err(StrValError::Serde)
            .and_then(|raw| from_json(&raw, t))?,
    };
    Ok(val)
}

#[allow(clippy::too_many_lines)]
pub fn from_json(v: &Value, t: &ScSpecTypeDef) -> Result<ScVal, StrValError> {
    let val: ScVal = match (t, v) {
        // Boolean parsing
        (ScSpecTypeDef::Bool, Value::Bool(true)) => ScVal::Static(ScStatic::True),
        (ScSpecTypeDef::Bool, Value::Bool(false)) => ScVal::Static(ScStatic::False),

        // Vec parsing
        (ScSpecTypeDef::Vec(elem), Value::Array(raw)) => {
            let ScSpecTypeVec { element_type } = &**elem;
            let parsed: Result<Vec<ScVal>, StrValError> = raw
                .iter()
                .map(|item| -> Result<ScVal, StrValError> { from_json(item, element_type) })
                .collect();
            let converted: ScVec = parsed?.try_into().map_err(StrValError::Xdr)?;
            ScVal::Object(Some(ScObject::Vec(converted)))
        }

        // Number parsing
        (ScSpecTypeDef::BigInt, Value::String(s)) => from_string(s, &ScSpecTypeDef::BigInt)?,
        (ScSpecTypeDef::BigInt, Value::Number(n)) => {
            from_json(&Value::String(format!("{}", n)), &ScSpecTypeDef::BigInt)?
        }
        (ScSpecTypeDef::I32, Value::Number(n)) => ScVal::I32(
            n.as_i64()
                .ok_or(StrValError::InvalidValue)?
                .try_into()
                .map_err(|_| StrValError::InvalidValue)?,
        ),
        (ScSpecTypeDef::I64, Value::Number(n)) => ScVal::Object(Some(ScObject::I64(
            n.as_i64().ok_or(StrValError::InvalidValue)?,
        ))),
        (ScSpecTypeDef::U32, Value::Number(n)) => ScVal::U32(
            n.as_u64()
                .ok_or(StrValError::InvalidValue)?
                .try_into()
                .map_err(|_| StrValError::InvalidValue)?,
        ),
        (ScSpecTypeDef::U64, Value::Number(n)) => {
            ScVal::U63(n.as_i64().ok_or(StrValError::InvalidValue)?)
        }

        // Map parsing
        (ScSpecTypeDef::Map(map), Value::Object(raw)) => {
            let ScSpecTypeMap {
                key_type,
                value_type,
            } = &**map;
            // TODO: What do we do if the expected key_type is not a string or symbol?
            let parsed: Result<Vec<ScMapEntry>, StrValError> = raw
                .iter()
                .map(|(k, v)| -> Result<ScMapEntry, StrValError> {
                    let key = from_string(k, key_type)?;
                    let val = from_json(v, value_type)?;
                    Ok(ScMapEntry { key, val })
                })
                .collect();
            ScVal::Object(Some(ScObject::Map(
                ScMap::sorted_from(parsed?).map_err(StrValError::Xdr)?,
            )))
        }

        // Symbol parsing
        (ScSpecTypeDef::Symbol, Value::String(s)) => ScVal::Symbol(
            s.as_bytes()
                .try_into()
                .map_err(|_| StrValError::InvalidValue)?,
        ),

        // Binary parsing
        (ScSpecTypeDef::Binary, Value::String(s)) => ScVal::Object(Some(ScObject::Binary(
            s.as_bytes()
                .try_into()
                .map_err(|_| StrValError::InvalidValue)?,
        ))),
        (ScSpecTypeDef::Binary, Value::Array(raw)) => {
            let b: Result<Vec<u8>, StrValError> = raw
                .iter()
                .map(|item| {
                    item.as_u64()
                        .ok_or(StrValError::InvalidValue)?
                        .try_into()
                        .map_err(|_| StrValError::InvalidValue)
                })
                .collect();
            let converted: VecM<u8, 256_000_u32> = b?.try_into().map_err(StrValError::Xdr)?;
            ScVal::Object(Some(ScObject::Binary(converted)))
        }

        // Option parsing
        // is null -> void the right thing here?
        (ScSpecTypeDef::Option(_), Value::Null) => ScVal::Object(None),
        (ScSpecTypeDef::Option(elem), v) => {
            let ScSpecTypeOption { value_type } = &**elem;
            ScVal::Object(Some(
                from_json(v, value_type)?
                    .try_into()
                    .map_err(|_| StrValError::InvalidValue)?,
            ))
        }

        // Tuple parsing
        (ScSpecTypeDef::Tuple(elem), Value::Array(raw)) => {
            let ScSpecTypeTuple { value_types } = &**elem;
            if raw.len() != value_types.len() {
                return Err(StrValError::InvalidValue);
            };
            let parsed: Result<Vec<ScVal>, StrValError> = raw
                .iter()
                .zip(value_types.iter())
                .map(|(item, t)| from_json(item, t))
                .collect();
            let converted: ScVec = parsed?.try_into().map_err(StrValError::Xdr)?;
            ScVal::Object(Some(ScObject::Vec(converted)))
        }

        // TODO: Implement the rest of these
        // ScSpecTypeDef::Bitset => {},
        // ScSpecTypeDef::Status => {},
        // ScSpecTypeDef::Result(Box<ScSpecTypeResult>) => {},
        // ScSpecTypeDef::Set(Box<ScSpecTypeSet>) => {},
        // ScSpecTypeDef::Udt(ScSpecTypeUdt) => {},
        (_, raw) => serde_json::from_value(raw.clone()).map_err(StrValError::Serde)?,
    };
    Ok(val)
}

pub fn to_string(v: &ScVal) -> Result<String, StrValError> {
    #[allow(clippy::match_same_arms)]
    Ok(match v {
        // If symbols are a top-level thing we omit the wrapping quotes
        // TODO: Decide if this is a good idea or not.
        ScVal::Symbol(v) => std::str::from_utf8(v.as_slice())
            .map_err(|_| StrValError::InvalidValue)?
            .to_string(),
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
            ScStatic::LedgerKeyContractCodeWasm => return Err(StrValError::InvalidValue),
        },
        ScVal::U63(v) => Value::Number(serde_json::Number::from(*v)),
        ScVal::U32(v) => Value::Number(serde_json::Number::from(*v)),
        ScVal::I32(v) => Value::Number(serde_json::Number::from(*v)),
        ScVal::Symbol(v) => Value::String(
            std::str::from_utf8(v.as_slice())
                .map_err(|_| StrValError::InvalidValue)?
                .to_string(),
        ),
        ScVal::Object(None) => Value::Null,
        ScVal::Object(Some(ScObject::Vec(v))) => {
            let values: Result<Vec<Value>, StrValError> = v
                .iter()
                .map(|item| -> Result<Value, StrValError> { to_json(item) })
                .collect();
            Value::Array(values?)
        }
        ScVal::Object(Some(ScObject::Map(v))) => {
            // TODO: What do we do if the key is not a string?
            let mut m = serde_json::Map::<String, Value>::with_capacity(v.len());
            for ScMapEntry { key, val } in v.iter() {
                let k: String = to_string(key)?;
                let v: Value = to_json(val).map_err(|_| StrValError::InvalidValue)?;
                m.insert(k, v);
            }
            Value::Object(m)
        }
        // TODO: Number is not the best choice here, because json parsers in clients might only
        // handle 53-bit numbers.
        ScVal::Object(Some(ScObject::U64(v))) => Value::Number(serde_json::Number::from(*v)),
        ScVal::Object(Some(ScObject::I64(v))) => Value::Number(serde_json::Number::from(*v)),
        ScVal::Object(Some(ScObject::Binary(v))) => Value::Array(
            v.to_vec()
                .iter()
                .map(|item| Value::Number(serde_json::Number::from(*item)))
                .collect(),
        ),
        ScVal::Object(Some(ScObject::BigInt(n))) => {
            // Always output bigints as strings
            Value::String(match n {
                ScBigInt::Zero => "0".to_string(),
                ScBigInt::Negative(bytes) => {
                    BigInt::from_bytes_be(Sign::Minus, bytes.as_ref()).to_str_radix(10)
                }
                ScBigInt::Positive(bytes) => {
                    BigInt::from_bytes_be(Sign::Plus, bytes.as_ref()).to_str_radix(10)
                }
            })
        }
        // TODO: Implement these
        ScVal::Object(Some(ScObject::Hash(_) | ScObject::PublicKey(_)))
        | ScVal::Bitset(_)
        | ScVal::Status(_) => serde_json::to_value(v).map_err(StrValError::Serde)?,
    };
    Ok(val)
}
