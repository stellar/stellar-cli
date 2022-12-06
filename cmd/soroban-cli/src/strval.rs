use serde_json::Value;
use std::{error::Error, fmt::Display, str::FromStr};

use soroban_env_host::xdr::{
    AccountId, BytesM, Error as XdrError, PublicKey, ScMap, ScMapEntry, ScObject, ScSpecTypeDef,
    ScSpecTypeMap, ScSpecTypeOption, ScSpecTypeTuple, ScSpecTypeVec, ScStatic, ScVal, ScVec,
    Uint256,
};

use stellar_strkey::StrkeyPublicKeyEd25519;

use crate::utils;

#[derive(Debug)]
pub enum StrValError {
    UnknownError,
    InvalidValue(Option<ScSpecTypeDef>),
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
            Self::InvalidValue(Some(s)) => write!(f, "value is not parseable to {:?}", s)?,
            Self::InvalidValue(None) => write!(f, "value is not parseable to type")?,
            Self::Serde(e) => write!(f, "{e}")?,
            Self::Xdr(e) => write!(f, "{e}")?,
        };
        Ok(())
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
                .map_err(|_| StrValError::InvalidValue(Some(t.clone())))?,
        ),

        // This might either be a json array of u8s, or just the raw utf-8 bytes
        ScSpecTypeDef::Bytes | ScSpecTypeDef::BytesN(_) => {
            match serde_json::from_str(s) {
                // First, see if it is a json array
                Ok(v @ (Value::Array(_) | Value::String(_))) => from_json(&v, t)?,
                _ => from_json(&Value::String(s.to_string()), t)?,
            }
        }

        ScSpecTypeDef::U128 => {
            if let Ok(Value::String(raw)) = serde_json::from_str(s) {
                // First, see if it is a json string, strip the quotes and recurse
                from_string(&raw, t)?
            } else {
                u128::from_str(s)
                    .map_err(|_| StrValError::InvalidValue(Some(t.clone())))?
                    .into()
            }
        }

        // Might have wrapping quotes if it is negative. e.g. "-5"
        ScSpecTypeDef::I128 => {
            if let Ok(Value::String(raw)) = serde_json::from_str(s) {
                // First, see if it is a json string, strip the quotes and recurse
                from_string(&raw, t)?
            } else {
                i128::from_str(s)
                    .map_err(|_| StrValError::InvalidValue(Some(t.clone())))?
                    .into()
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
        (ScSpecTypeDef::U128 | ScSpecTypeDef::I128, Value::String(s)) => from_string(s, t)?,
        (ScSpecTypeDef::U128, Value::Number(n)) => {
            let val: u128 = n
                .as_u64()
                .ok_or_else(|| StrValError::InvalidValue(Some(t.clone())))?
                .into();
            ScVal::Object(Some(val.into()))
        }
        (ScSpecTypeDef::I128, Value::Number(n)) => {
            let val: i128 = n
                .as_i64()
                .ok_or_else(|| StrValError::InvalidValue(Some(t.clone())))?
                .into();
            ScVal::Object(Some(val.into()))
        }
        (ScSpecTypeDef::I32, Value::Number(n)) => ScVal::I32(
            n.as_i64()
                .ok_or_else(|| StrValError::InvalidValue(Some(t.clone())))?
                .try_into()
                .map_err(|_| StrValError::InvalidValue(Some(t.clone())))?,
        ),
        (ScSpecTypeDef::I64, Value::Number(n)) => ScVal::Object(Some(ScObject::I64(
            n.as_i64()
                .ok_or_else(|| StrValError::InvalidValue(Some(t.clone())))?,
        ))),
        (ScSpecTypeDef::U32, Value::Number(n)) => ScVal::U32(
            n.as_u64()
                .ok_or_else(|| StrValError::InvalidValue(Some(t.clone())))?
                .try_into()
                .map_err(|_| StrValError::InvalidValue(Some(t.clone())))?,
        ),
        (ScSpecTypeDef::U64, Value::Number(n)) => ScVal::Object(Some(ScObject::U64(
            n.as_u64()
                .ok_or_else(|| StrValError::InvalidValue(Some(t.clone())))?,
        ))),

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
                .map_err(|_| StrValError::InvalidValue(Some(t.clone())))?,
        ),

        // AccountID parsing
        (ScSpecTypeDef::AccountId, Value::String(s)) => ScVal::Object(Some(ScObject::AccountId({
            StrkeyPublicKeyEd25519::from_string(s)
                .map(|key| AccountId(PublicKey::PublicKeyTypeEd25519(Uint256(key.0))))
                .map_err(|_| StrValError::InvalidValue(Some(t.clone())))?
        }))),

        // Bytes parsing
        (ScSpecTypeDef::BytesN(bytes), Value::String(s)) => ScVal::Object(Some(ScObject::Bytes({
            if let Ok(key) = StrkeyPublicKeyEd25519::from_string(s) {
                key.0
                    .try_into()
                    .map_err(|_| StrValError::InvalidValue(Some(t.clone())))?
            } else {
                utils::padded_hex_from_str(s, bytes.n as usize)
                    .map_err(|_| StrValError::InvalidValue(Some(t.clone())))?
                    .try_into()
                    .map_err(|_| StrValError::InvalidValue(Some(t.clone())))?
            }
        }))),
        (ScSpecTypeDef::Bytes, Value::String(s)) => ScVal::Object(Some(ScObject::Bytes(
            hex::decode(s)
                .map_err(|_| StrValError::InvalidValue(Some(t.clone())))?
                .try_into()
                .map_err(|_| StrValError::InvalidValue(Some(t.clone())))?,
        ))),
        (ScSpecTypeDef::Bytes | ScSpecTypeDef::BytesN(_), Value::Array(raw)) => {
            let b: Result<Vec<u8>, StrValError> = raw
                .iter()
                .map(|item| {
                    item.as_u64()
                        .ok_or_else(|| StrValError::InvalidValue(Some(t.clone())))?
                        .try_into()
                        .map_err(|_| StrValError::InvalidValue(Some(t.clone())))
                })
                .collect();
            let converted: BytesM<256_000_u32> = b?.try_into().map_err(StrValError::Xdr)?;
            ScVal::Object(Some(ScObject::Bytes(converted)))
        }

        // Option parsing
        // is null -> void the right thing here?
        (ScSpecTypeDef::Option(_), Value::Null) => ScVal::Object(None),
        (ScSpecTypeDef::Option(elem), v) => {
            let ScSpecTypeOption { value_type } = &**elem;
            ScVal::Object(Some(
                from_json(v, value_type)?
                    .try_into()
                    .map_err(|_| StrValError::InvalidValue(Some(t.clone())))?,
            ))
        }

        // Tuple parsing
        (ScSpecTypeDef::Tuple(elem), Value::Array(raw)) => {
            let ScSpecTypeTuple { value_types } = &**elem;
            if raw.len() != value_types.len() {
                return Err(StrValError::InvalidValue(Some(t.clone())));
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
            .map_err(|_| StrValError::InvalidValue(Some(ScSpecTypeDef::Symbol)))?
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
            ScStatic::LedgerKeyContractCode => return Err(StrValError::InvalidValue(None)),
        },
        ScVal::U63(v) => Value::Number(serde_json::Number::from(*v)),
        ScVal::U32(v) => Value::Number(serde_json::Number::from(*v)),
        ScVal::I32(v) => Value::Number(serde_json::Number::from(*v)),
        ScVal::Symbol(v) => Value::String(
            std::str::from_utf8(v.as_slice())
                .map_err(|_| StrValError::InvalidValue(Some(ScSpecTypeDef::Symbol)))?
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
                let v: Value = to_json(val).map_err(|_| StrValError::InvalidValue(None))?;
                m.insert(k, v);
            }
            Value::Object(m)
        }
        // TODO: Number is not the best choice here, because json parsers in clients might only
        // handle 53-bit numbers.
        ScVal::Object(Some(ScObject::U64(v))) => Value::Number(serde_json::Number::from(*v)),
        ScVal::Object(Some(ScObject::I64(v))) => Value::Number(serde_json::Number::from(*v)),
        ScVal::Object(Some(ScObject::Bytes(v))) => Value::Array(
            v.to_vec()
                .iter()
                .map(|item| Value::Number(serde_json::Number::from(*item)))
                .collect(),
        ),
        ScVal::Object(Some(ScObject::AccountId(v))) => match v {
            AccountId(PublicKey::PublicKeyTypeEd25519(Uint256(k))) => {
                Value::String(StrkeyPublicKeyEd25519(*k).to_string())
            }
        },
        ScVal::Object(Some(ScObject::U128(n))) => {
            // Always output u128s as strings
            let v: u128 = ScObject::U128(n.clone())
                .try_into()
                .map_err(|_| StrValError::InvalidValue(Some(ScSpecTypeDef::U128)))?;
            Value::String(v.to_string())
        }
        ScVal::Object(Some(ScObject::I128(n))) => {
            // Always output i128s as strings
            let v: i128 = ScObject::I128(n.clone())
                .try_into()
                .map_err(|_| StrValError::InvalidValue(Some(ScSpecTypeDef::I128)))?;
            Value::String(v.to_string())
        }
        // TODO: Implement these
        ScVal::Object(Some(ScObject::ContractCode(_))) | ScVal::Bitset(_) | ScVal::Status(_) => {
            serde_json::to_value(v).map_err(StrValError::Serde)?
        }
    };
    Ok(val)
}
