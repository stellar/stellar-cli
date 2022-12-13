use serde_json::Value;
use std::str::FromStr;

use soroban_env_host::xdr::{
    AccountId, BytesM, Error as XdrError, PublicKey, ScMap, ScMapEntry, ScObject, ScSpecEntry,
    ScSpecFunctionV0, ScSpecTypeDef, ScSpecTypeMap, ScSpecTypeTuple, ScSpecTypeUdt,
    ScSpecUdtEnumV0, ScSpecUdtStructV0, ScSpecUdtUnionV0, ScStatic, ScVal, ScVec, StringM, Uint256,
};

use stellar_strkey::StrkeyPublicKeyEd25519;

use crate::utils;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("an unknown error occurred")]
    Unknown,
    #[error("value is not parseable to {0:#?}")]
    InvalidValue(Option<ScSpecTypeDef>),
    #[error("Unknown case {0} for {1}")]
    EnumCase(String, String),
    #[error("Unknown const case {0}")]
    EnumConst(u32),
    #[error("Enum const value must be a u32 or smaller")]
    EnumConstTooLarge(u64),
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
        // Parse as string and for special types assume Value::String
        serde_json::from_str(s)
            .or_else(|e| match t {
                ScSpecTypeDef::Symbol
                | ScSpecTypeDef::Bytes
                | ScSpecTypeDef::BytesN(_)
                | ScSpecTypeDef::U128
                | ScSpecTypeDef::I128
                | ScSpecTypeDef::AccountId => Ok(Value::String(s.to_owned())),
                ScSpecTypeDef::Udt(ScSpecTypeUdt { name })
                    if matches!(
                        self.find(&name.to_string_lossy())?,
                        ScSpecEntry::UdtUnionV0(_) | ScSpecEntry::UdtStructV0(_)
                    ) =>
                {
                    Ok(Value::String(s.to_owned()))
                }
                _ => Err(Error::Serde(e)),
            })
            .and_then(|raw| self.from_json(&raw, t))
    }

    #[allow(clippy::wrong_self_convention)]
    pub fn from_json(&self, v: &Value, t: &ScSpecTypeDef) -> Result<ScVal, Error> {
        let val: ScVal = match (t, v) {
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

            // Vec parsing
            (ScSpecTypeDef::Vec(elem), Value::Array(raw)) => {
                let converted: ScVec = raw
                    .iter()
                    .map(|item| self.from_json(item, &elem.element_type))
                    .collect::<Result<Vec<ScVal>, Error>>()?
                    .try_into()
                    .map_err(Error::Xdr)?;
                ScVal::Object(Some(ScObject::Vec(converted)))
            }

            // Map parsing
            (ScSpecTypeDef::Map(map), Value::Object(raw)) => self.parse_map(map, raw)?,

            // Option parsing
            // is null -> void the right thing here?
            (ScSpecTypeDef::Option(_), Value::Null) => ScVal::Object(None),
            (ScSpecTypeDef::Option(elem), v) => ScVal::Object(Some(
                self.from_json(v, &elem.value_type)?
                    .try_into()
                    .map_err(|_| Error::InvalidValue(Some(t.clone())))?,
            )),

            // Tuple parsing
            (ScSpecTypeDef::Tuple(elem), Value::Array(raw)) => self.parse_tuple(t, elem, raw)?,

            (ScSpecTypeDef::Udt(ScSpecTypeUdt { name }), _) => self.parse_udt(name, v)?,

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

    fn parse_udt(&self, name: &StringM<60>, value: &Value) -> Result<ScVal, Error> {
        let name = &name.to_string_lossy();
        match (self.find(name)?, value) {
            (ScSpecEntry::UdtStructV0(strukt), Value::Object(map)) => {
                self.parse_strukt(strukt, map)
            }
            (ScSpecEntry::UdtStructV0(strukt), Value::Array(arr)) => {
                self.parse_tuple_strukt(strukt, arr)
            }
            (ScSpecEntry::UdtUnionV0(union), val @ (Value::String(_) | Value::Object(_))) => {
                self.parse_union(union, val)
            }
            (ScSpecEntry::UdtEnumV0(enum_), Value::Number(num)) => parse_const_enum(num, enum_),
            (s, v) => todo!("Not implemented for {s:#?} {v:#?}"),
        }
    }

    fn parse_tuple_strukt(
        &self,
        strukt: &ScSpecUdtStructV0,
        array: &[Value],
    ) -> Result<ScVal, Error> {
        let items = strukt
            .fields
            .to_vec()
            .iter()
            .zip(array.iter())
            .map(|(f, v)| {
                let val = self.from_json(v, &f.type_)?;
                Ok(val)
            })
            .collect::<Result<Vec<_>, Error>>()?;
        Ok(ScVal::Object(Some(ScObject::Vec(
            items.try_into().map_err(Error::Xdr)?,
        ))))
    }

    fn parse_strukt(
        &self,
        strukt: &ScSpecUdtStructV0,
        map: &serde_json::Map<String, Value>,
    ) -> Result<ScVal, Error> {
        let items = strukt
            .fields
            .to_vec()
            .iter()
            .map(|f| {
                let name = &f.name.to_string_lossy();
                let v = map.get(name).ok_or(Error::Unknown)?;
                let val = self.from_json(v, &f.type_)?;
                let key = StringM::from_str(name).unwrap();
                Ok(ScMapEntry {
                    key: ScVal::Symbol(key),
                    val,
                })
            })
            .collect::<Result<Vec<_>, Error>>()?;
        let map = ScMap::sorted_from(items).map_err(Error::Xdr)?;
        Ok(ScVal::Object(Some(ScObject::Map(map))))
    }

    fn parse_union(&self, union: &ScSpecUdtUnionV0, value: &Value) -> Result<ScVal, Error> {
        let (enum_case, kind) = match value {
            Value::String(s) => (s, None),
            Value::Object(o) if o.len() == 1 => (o.keys().next().unwrap(), o.values().next()),
            _ => todo!(),
        };
        let (case, type_) = union
            .cases
            .to_vec()
            .iter()
            .find(|c| enum_case == &c.name.to_string_lossy())
            .map(|c| (c.name.to_string_lossy(), c.type_.clone()))
            .ok_or_else(|| Error::EnumCase(enum_case.to_string(), union.name.to_string_lossy()))?;
        let s_vec = if let Some(value) = kind {
            let val = self.from_json(value, type_.as_ref().unwrap())?;
            let key = ScVal::Symbol(StringM::from_str(enum_case).map_err(Error::Xdr)?);
            vec![key, val]
            // let map = ScMap::sorted_from(vec![ScMapEntry { key, val }]).map_err(Error::Xdr)?;
        } else {
            let val = ScVal::Symbol(case.try_into().map_err(Error::Xdr)?);
            vec![val]
        };
        Ok(ScVal::Object(Some(ScObject::Vec(
            s_vec.try_into().map_err(Error::Xdr)?,
        ))))
    }

    fn parse_tuple(
        &self,
        t: &ScSpecTypeDef,
        tuple: &ScSpecTypeTuple,
        items: &[Value],
    ) -> Result<ScVal, Error> {
        let ScSpecTypeTuple { value_types } = tuple;
        if items.len() != value_types.len() {
            return Err(Error::InvalidValue(Some(t.clone())));
        };
        let parsed: Result<Vec<ScVal>, Error> = items
            .iter()
            .zip(value_types.iter())
            .map(|(item, t)| self.from_json(item, t))
            .collect();
        let converted: ScVec = parsed?.try_into().map_err(Error::Xdr)?;
        Ok(ScVal::Object(Some(ScObject::Vec(converted))))
    }

    fn parse_map(
        &self,
        map: &ScSpecTypeMap,
        value_map: &serde_json::Map<String, Value>,
    ) -> Result<ScVal, Error> {
        let ScSpecTypeMap {
            key_type,
            value_type,
        } = map;
        // TODO: What do we do if the expected key_type is not a string or symbol?
        let parsed: Result<Vec<ScMapEntry>, Error> = value_map
            .iter()
            .map(|(k, v)| -> Result<ScMapEntry, Error> {
                let key = self.from_string(k, key_type)?;
                let val = self.from_json(v, value_type)?;
                Ok(ScMapEntry { key, val })
            })
            .collect();
        Ok(ScVal::Object(Some(ScObject::Map(
            ScMap::sorted_from(parsed?).map_err(Error::Xdr)?,
        ))))
    }
}

pub fn from_string_primitive(s: &str, t: &ScSpecTypeDef) -> Result<ScVal, Error> {
    Spec::from_string_primitive(s, t)
}

fn parse_const_enum(num: &serde_json::Number, enum_: &ScSpecUdtEnumV0) -> Result<ScVal, Error> {
    let num = num.as_u64().ok_or(Error::Unknown)?;
    let num = u32::try_from(num).map_err(|_| Error::EnumConstTooLarge(num))?;
    enum_
        .cases
        .iter()
        .find(|c| c.value == num)
        .ok_or(Error::EnumConst(num))
        .map(|c| ScVal::U32(c.value))
}

pub fn from_json_primitives(v: &Value, t: &ScSpecTypeDef) -> Result<ScVal, Error> {
    let val: ScVal = match (t, v) {
        // Boolean parsing
        (ScSpecTypeDef::Bool, Value::Bool(true)) => ScVal::Static(ScStatic::True),
        (ScSpecTypeDef::Bool, Value::Bool(false)) => ScVal::Static(ScStatic::False),

        // Number parsing
        // TODO: Decide if numbers are appropriate for (i/u)128
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
        (ScSpecTypeDef::U128, Value::String(s)) => {
            let val: u128 = u128::from_str(s)
                .map(Into::into)
                .map_err(|_| Error::InvalidValue(Some(t.clone())))?;
            ScVal::Object(Some(val.into()))
        }

        (ScSpecTypeDef::I128, Value::String(s)) => {
            let val: i128 = i128::from_str(s)
                .map(Into::into)
                .map_err(|_| Error::InvalidValue(Some(t.clone())))?;
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

        // Bytes parsing
        (ScSpecTypeDef::BytesN(bytes), Value::String(s)) => ScVal::Object(Some(ScObject::Bytes({
            if let Ok(key) = StrkeyPublicKeyEd25519::from_string(s) {
                key.0
                    .try_into()
                    .map_err(|_| Error::InvalidValue(Some(t.clone())))?
            } else {
                utils::padded_hex_from_str(s, bytes.n as usize)
                    .map_err(|_| Error::InvalidValue(Some(t.clone())))?
                    .try_into()
                    .map_err(|_| Error::InvalidValue(Some(t.clone())))?
            }
        }))),
        (ScSpecTypeDef::Bytes, Value::String(s)) => ScVal::Object(Some(ScObject::Bytes(
            hex::decode(s)
                .map_err(|_| Error::InvalidValue(Some(t.clone())))?
                .try_into()
                .map_err(|_| Error::InvalidValue(Some(t.clone())))?,
        ))),
        (ScSpecTypeDef::Bytes | ScSpecTypeDef::BytesN(_), Value::Array(raw)) => {
            let b: Result<Vec<u8>, Error> = raw
                .iter()
                .map(|item| {
                    item.as_u64()
                        .ok_or_else(|| Error::InvalidValue(Some(t.clone())))?
                        .try_into()
                        .map_err(|_| Error::InvalidValue(Some(t.clone())))
                })
                .collect();
            let converted: BytesM<256_000_u32> = b?.try_into().map_err(Error::Xdr)?;
            ScVal::Object(Some(ScObject::Bytes(converted)))
        }

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
