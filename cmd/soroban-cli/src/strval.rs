use serde_json::Value;
use std::str::FromStr;

use soroban_env_host::xdr::{
    AccountId, Error as XdrError, PublicKey, ScMap, ScMapEntry, ScObject, ScSpecEntry,
    ScSpecFunctionV0, ScSpecTypeDef, ScSpecTypeMap, ScSpecTypeOption, ScSpecTypeTuple,
    ScSpecTypeUdt, ScSpecTypeVec, ScSpecUdtStructV0, ScStatic, ScVal, ScVec, StringM, Uint256,
};

use stellar_strkey::StrkeyPublicKeyEd25519;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("an unknown error occurred")]
    Unknown,
    #[error("value is not parseable to {0:#?}")]
    InvalidValue(Option<ScSpecTypeDef>),
    #[error("Unknown case {0} for {1}")]
    EnumCase(String, String),
    #[error("Missing Entry {0}")]
    MissingEntry(String),
    #[error(transparent)]
    Xdr(XdrError),
    #[error(transparent)]
    Serde(serde_json::Error),
}

impl From<()> for Error {
    fn from(_: ()) -> Self {
        Error::Unknown
    }
}

#[derive(Default)]
pub struct Spec(pub Option<Vec<ScSpecEntry>>);

impl Spec {
    pub fn find(&self, name: &str) -> Result<&ScSpecEntry, Error> {
        self.0
            .as_ref()
            .and_then(|specs| {
                specs.iter().find(|e| {
                    let entry_name = match e {
                        ScSpecEntry::FunctionV0(x) => x.name.to_string_lossy(),
                        ScSpecEntry::UdtStructV0(x) => x.name.to_string_lossy(),
                        ScSpecEntry::UdtUnionV0(x) => x.name.to_string_lossy(),
                        ScSpecEntry::UdtEnumV0(x) => x.name.to_string_lossy(),
                        ScSpecEntry::UdtErrorEnumV0(x) => x.name.to_string_lossy(),
                    };
                    name == entry_name
                })
            })
            .ok_or_else(|| Error::MissingEntry(name.to_owned()))
    }

    pub fn find_function(&self, name: &str) -> Result<&ScSpecFunctionV0, Error> {
        match self.find(name)? {
            ScSpecEntry::FunctionV0(f) => Ok(f),
            _ => Err(Error::MissingEntry(name.to_owned())),
        }
    }
    pub fn from_string_primitive(s: &str, t: &ScSpecTypeDef) -> Result<ScVal, Error> {
        Self::default().from_string(s, t)
    }

    #[allow(clippy::wrong_self_convention)]
    pub fn from_string(&self, s: &str, t: &ScSpecTypeDef) -> Result<ScVal, Error> {
        match t {
            // These ones have special processing when they're the top-level args. This is so we don't
            // need extra quotes around string args.
            ScSpecTypeDef::Symbol => s
                .as_bytes()
                .try_into()
                .map_err(|_| Error::InvalidValue(Some(t.clone())))
                .map(ScVal::Symbol),

            // This might either be a json array of u8s, or just the raw utf-8 bytes
            ScSpecTypeDef::Bytes | ScSpecTypeDef::BytesN(_) => {
                match serde_json::from_str(s) {
                    // First, see if it is a json array
                    Ok(v @ (Value::Array(_) | Value::String(_))) => self.from_json(&v, t),
                    _ => self.from_json(&Value::String(s.to_string()), t),
                }
            }

            ScSpecTypeDef::U128 => {
                if let Ok(Value::String(raw)) = serde_json::from_str(s) {
                    // First, see if it is a json string, strip the quotes and recurse
                    self.from_string(&raw, t)
                } else {
                    u128::from_str(s)
                        .map(Into::into)
                        .map_err(|_| Error::InvalidValue(Some(t.clone())))
                }
            }

            // Might have wrapping quotes if it is negative. e.g. "-5"
            ScSpecTypeDef::I128 => {
                if let Ok(Value::String(raw)) = serde_json::from_str(s) {
                    // First, see if it is a json string, strip the quotes and recurse
                    self.from_string(&raw, t)
                } else {
                    i128::from_str(s)
                        .map(Into::into)
                        .map_err(|_| Error::InvalidValue(Some(t.clone())))
                }
            }
            ScSpecTypeDef::Udt(ScSpecTypeUdt { name })
                if matches!(
                    self.find(&name.to_string_lossy())?,
                    ScSpecEntry::UdtUnionV0(_)
                ) =>
            {
                self.from_json(&Value::String(s.to_string()), t)
            }

            // For all others we just use the json parser
            _ => serde_json::from_str(s)
                .map_err(Error::Serde)
                .and_then(|raw| self.from_json(&raw, t)),
        }
    }

    #[allow(clippy::wrong_self_convention)]
    pub fn from_json(&self, v: &Value, t: &ScSpecTypeDef) -> Result<ScVal, Error> {
        match (t, v) {
            // Boolean parsing
            (
                ScSpecTypeDef::Bool
                | ScSpecTypeDef::U128
                | ScSpecTypeDef::I128
                | ScSpecTypeDef::I32
                | ScSpecTypeDef::I64
                | ScSpecTypeDef::U32
                | ScSpecTypeDef::U64
                | ScSpecTypeDef::Symbol
                | ScSpecTypeDef::AccountId
                | ScSpecTypeDef::Bytes
                | ScSpecTypeDef::BytesN(_),
                _,
            ) => from_json_primitives(v, t),

            _ => self.from_json_complex(v, t),
            // // TODO: Implement the rest of these
            // // ScSpecTypeDef::Bitset => {},
            // // ScSpecTypeDef::Status => {},
            // // ScSpecTypeDef::Result(Box<ScSpecTypeResult>) => {},
            // // ScSpecTypeDef::Set(Box<ScSpecTypeSet>) => {},
            // // ScSpecTypeDef::Udt(ScSpecTypeUdt) => {},
            // (_, raw) => serde_json::from_value(raw.clone()).map_err(Error::Serde)?,
        }
    }

    #[allow(clippy::too_many_lines, clippy::wrong_self_convention)]
    pub fn from_json_complex(&self, v: &Value, t: &ScSpecTypeDef) -> Result<ScVal, Error> {
        let val: ScVal = match (t, v) {
            // Boolean parsing
            (
                ScSpecTypeDef::Bool
                | ScSpecTypeDef::U128
                | ScSpecTypeDef::I128
                | ScSpecTypeDef::I32
                | ScSpecTypeDef::I64
                | ScSpecTypeDef::U32
                | ScSpecTypeDef::U64
                | ScSpecTypeDef::Symbol
                | ScSpecTypeDef::AccountId
                | ScSpecTypeDef::Bytes
                | ScSpecTypeDef::BytesN(_),
                _,
            ) => from_json_primitives(v, t)?,

            (ScSpecTypeDef::Udt(ScSpecTypeUdt { name }), Value::Object(o)) => {
                let type_ = self.find(&name.to_string_lossy())?;
                let items = match type_ {
                    ScSpecEntry::UdtStructV0(ScSpecUdtStructV0 { fields, .. }) => fields
                        .to_vec()
                        .iter()
                        .map(|f| {
                            let name = &f.name.to_string_lossy();
                            let v = o.get(name).ok_or(Error::Unknown)?;
                            let val = self.from_json(v, &f.type_)?;
                            let key = StringM::from_str(name).unwrap();
                            Ok(ScMapEntry {
                                key: ScVal::Symbol(key),
                                val,
                            })
                        })
                        .collect::<Result<Vec<_>, Error>>()?,
                    ScSpecEntry::FunctionV0(_) => todo!(),
                    ScSpecEntry::UdtUnionV0(_union_) => todo!(),
                    ScSpecEntry::UdtEnumV0(_) => todo!(),
                    ScSpecEntry::UdtErrorEnumV0(_) => todo!(),
                };
                let map = ScMap::sorted_from(items).map_err(Error::Xdr)?;

                ScVal::Object(Some(ScObject::Map(map)))
            }
            (ScSpecTypeDef::Udt(ScSpecTypeUdt { name }), Value::String(s)) => {
                let case = match self.find(&name.to_string_lossy())? {
                    ScSpecEntry::UdtUnionV0(union_) => union_
                        .cases
                        .to_vec()
                        .iter()
                        .find(|c| s == &c.name.to_string_lossy())
                        .map(|c| c.name.to_string_lossy()),
                    ScSpecEntry::FunctionV0(_)
                    | ScSpecEntry::UdtStructV0(_)
                    | ScSpecEntry::UdtEnumV0(_)
                    | ScSpecEntry::UdtErrorEnumV0(_) => todo!(),
                }
                .ok_or_else(|| Error::EnumCase(s.to_string(), name.to_string_lossy()))?;
                let val = ScVal::Symbol(case.try_into().map_err(Error::Xdr)?);
                let s_vec = vec![val];
                let s_vec = s_vec.try_into().map_err(Error::Xdr)?;
                ScVal::Object(Some(ScObject::Vec(s_vec)))
            }

            // Vec parsing
            (ScSpecTypeDef::Vec(elem), Value::Array(raw)) => {
                let ScSpecTypeVec { element_type } = &**elem;
                let parsed: Result<Vec<ScVal>, Error> = raw
                    .iter()
                    .map(|item| -> Result<ScVal, Error> { self.from_json(item, element_type) })
                    .collect();
                let converted: ScVec = parsed?.try_into().map_err(Error::Xdr)?;
                ScVal::Object(Some(ScObject::Vec(converted)))
            }

            // Map parsing
            (ScSpecTypeDef::Map(map), Value::Object(raw)) => {
                let ScSpecTypeMap {
                    key_type,
                    value_type,
                } = &**map;
                // TODO: What do we do if the expected key_type is not a string or symbol?
                let parsed: Result<Vec<ScMapEntry>, Error> = raw
                    .iter()
                    .map(|(k, v)| -> Result<ScMapEntry, Error> {
                        let key = self.from_string(k, key_type)?;
                        let val = self.from_json(v, value_type)?;
                        Ok(ScMapEntry { key, val })
                    })
                    .collect();
                ScVal::Object(Some(ScObject::Map(
                    ScMap::sorted_from(parsed?).map_err(Error::Xdr)?,
                )))
            }

            // Option parsing
            // is null -> void the right thing here?
            (ScSpecTypeDef::Option(_), Value::Null) => ScVal::Object(None),
            (ScSpecTypeDef::Option(elem), v) => {
                let ScSpecTypeOption { value_type } = &**elem;
                ScVal::Object(Some(
                    self.from_json(v, value_type)?
                        .try_into()
                        .map_err(|_| Error::InvalidValue(Some(t.clone())))?,
                ))
            }

            // Tuple parsing
            (ScSpecTypeDef::Tuple(elem), Value::Array(raw)) => {
                let ScSpecTypeTuple { value_types } = &**elem;
                if raw.len() != value_types.len() {
                    return Err(Error::InvalidValue(Some(t.clone())));
                };
                let parsed: Result<Vec<ScVal>, Error> = raw
                    .iter()
                    .zip(value_types.iter())
                    .map(|(item, t)| self.from_json(item, t))
                    .collect();
                let converted: ScVec = parsed?.try_into().map_err(Error::Xdr)?;
                ScVal::Object(Some(ScObject::Vec(converted)))
            }

            // TODO: Implement the rest of these
            // ScSpecTypeDef::Bitset => {},
            // ScSpecTypeDef::Status => {},
            // ScSpecTypeDef::Result(Box<ScSpecTypeResult>) => {},
            // ScSpecTypeDef::Set(Box<ScSpecTypeSet>) => {},
            // ScSpecTypeDef::Udt(ScSpecTypeUdt) => {},
            (_, raw) => serde_json::from_value(raw.clone()).map_err(Error::Serde)?,
        };
        Ok(val)
    }
}

pub fn from_string_primitive(s: &str, t: &ScSpecTypeDef) -> Result<ScVal, Error> {
    Spec::from_string_primitive(s, t)
}

pub fn from_json_primitives(v: &Value, t: &ScSpecTypeDef) -> Result<ScVal, Error> {
    let val: ScVal = match (t, v) {
        // Boolean parsing
        (ScSpecTypeDef::Bool, Value::Bool(true)) => ScVal::Static(ScStatic::True),
        (ScSpecTypeDef::Bool, Value::Bool(false)) => ScVal::Static(ScStatic::False),

        // Number parsing
        (ScSpecTypeDef::U128 | ScSpecTypeDef::I128, Value::String(s)) => {
            from_string_primitive(s, t)?
        }
        (ScSpecTypeDef::U128, Value::Number(n)) => {
            let val: u128 = n
                .as_u64()
                .ok_or_else(|| Error::InvalidValue(Some(t.clone())))?
                .into();
            ScVal::Object(Some(val.into()))
        }
        (ScSpecTypeDef::I128, Value::Number(n)) => {
            let val: i128 = n
                .as_i64()
                .ok_or_else(|| Error::InvalidValue(Some(t.clone())))?
                .into();
            ScVal::Object(Some(val.into()))
        }
        (ScSpecTypeDef::I32, Value::Number(n)) => ScVal::I32(
            n.as_i64()
                .ok_or_else(|| Error::InvalidValue(Some(t.clone())))?
                .try_into()
                .map_err(|_| Error::InvalidValue(Some(t.clone())))?,
        ),
        (ScSpecTypeDef::I64, Value::Number(n)) => ScVal::Object(Some(ScObject::I64(
            n.as_i64()
                .ok_or_else(|| Error::InvalidValue(Some(t.clone())))?,
        ))),
        (ScSpecTypeDef::U32, Value::Number(n)) => ScVal::U32(
            n.as_u64()
                .ok_or_else(|| Error::InvalidValue(Some(t.clone())))?
                .try_into()
                .map_err(|_| Error::InvalidValue(Some(t.clone())))?,
        ),
        (ScSpecTypeDef::U64, Value::Number(n)) => ScVal::Object(Some(ScObject::U64(
            n.as_u64()
                .ok_or_else(|| Error::InvalidValue(Some(t.clone())))?,
        ))),

        // Symbol parsing
        (ScSpecTypeDef::Symbol, Value::String(s)) => ScVal::Symbol(
            s.as_bytes()
                .try_into()
                .map_err(|_| Error::InvalidValue(Some(t.clone())))?,
        ),

        // AccountID parsing
        (ScSpecTypeDef::AccountId, Value::String(s)) => ScVal::Object(Some(ScObject::AccountId({
            StrkeyPublicKeyEd25519::from_string(s)
                .map(|key| AccountId(PublicKey::PublicKeyTypeEd25519(Uint256(key.0))))
                .map_err(|_| Error::InvalidValue(Some(t.clone())))?
        }))),
        // Todo make proper error Which shouldn't exist
        (_, raw) => serde_json::from_value(raw.clone()).map_err(Error::Serde)?,
    };
    Ok(val)
}

pub fn to_string(v: &ScVal) -> Result<String, Error> {
    #[allow(clippy::match_same_arms)]
    Ok(match v {
        // If symbols are a top-level thing we omit the wrapping quotes
        // TODO: Decide if this is a good idea or not.
        ScVal::Symbol(v) => std::str::from_utf8(v.as_slice())
            .map_err(|_| Error::InvalidValue(Some(ScSpecTypeDef::Symbol)))?
            .to_string(),
        _ => serde_json::to_string(&to_json(v)?).map_err(Error::Serde)?,
    })
}

pub fn to_json(v: &ScVal) -> Result<Value, Error> {
    #[allow(clippy::match_same_arms)]
    let val: Value = match v {
        ScVal::Static(v) => match v {
            ScStatic::True => Value::Bool(true),
            ScStatic::False => Value::Bool(false),
            ScStatic::Void => Value::Null,
            ScStatic::LedgerKeyContractCode => return Err(Error::InvalidValue(None)),
        },
        ScVal::U63(v) => Value::Number(serde_json::Number::from(*v)),
        ScVal::U32(v) => Value::Number(serde_json::Number::from(*v)),
        ScVal::I32(v) => Value::Number(serde_json::Number::from(*v)),
        ScVal::Symbol(v) => Value::String(
            std::str::from_utf8(v.as_slice())
                .map_err(|_| Error::InvalidValue(Some(ScSpecTypeDef::Symbol)))?
                .to_string(),
        ),
        ScVal::Object(None) => Value::Null,
        ScVal::Object(Some(ScObject::Vec(v))) => {
            let values: Result<Vec<Value>, Error> = v
                .iter()
                .map(|item| -> Result<Value, Error> { to_json(item) })
                .collect();
            Value::Array(values?)
        }
        ScVal::Object(Some(ScObject::Map(v))) => {
            // TODO: What do we do if the key is not a string?
            let mut m = serde_json::Map::<String, Value>::with_capacity(v.len());
            for ScMapEntry { key, val } in v.iter() {
                let k: String = to_string(key)?;
                let v: Value = to_json(val).map_err(|_| Error::InvalidValue(None))?;
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
                .map_err(|_| Error::InvalidValue(Some(ScSpecTypeDef::U128)))?;
            Value::String(v.to_string())
        }
        ScVal::Object(Some(ScObject::I128(n))) => {
            // Always output i128s as strings
            let v: i128 = ScObject::I128(n.clone())
                .try_into()
                .map_err(|_| Error::InvalidValue(Some(ScSpecTypeDef::I128)))?;
            Value::String(v.to_string())
        }
        // TODO: Implement these
        ScVal::Object(Some(ScObject::ContractCode(_))) | ScVal::Bitset(_) | ScVal::Status(_) => {
            serde_json::to_value(v).map_err(Error::Serde)?
        }
    };
    Ok(val)
}
