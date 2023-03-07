use itertools::Itertools;
use serde_json::Value;
use std::str::FromStr;

use soroban_env_host::xdr::{
    self, AccountId, BytesM, Error as XdrError, Hash, PublicKey, ScAddress, ScMap, ScMapEntry,
    ScObject, ScSpecEntry, ScSpecFunctionV0, ScSpecTypeDef as ScType, ScSpecTypeMap,
    ScSpecTypeOption, ScSpecTypeResult, ScSpecTypeSet, ScSpecTypeTuple, ScSpecTypeUdt,
    ScSpecTypeVec, ScSpecUdtEnumV0, ScSpecUdtErrorEnumV0, ScSpecUdtStructV0,
    ScSpecUdtUnionCaseTupleV0, ScSpecUdtUnionCaseV0, ScSpecUdtUnionCaseVoidV0, ScSpecUdtUnionV0,
    ScStatic, ScVal, ScVec, StringM, Uint256, VecM,
};

use crate::utils;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("an unknown error occurred")]
    Unknown,
    #[error("value is not parseable to {0:#?}")]
    InvalidValue(Option<ScType>),
    #[error("Unknown case {0} for {1}")]
    EnumCase(String, String),
    #[error("Enum {0} missing value for type {1}")]
    EnumMissingSecondValue(String, String),
    #[error("Unknown const case {0}")]
    EnumConst(u32),
    #[error("Enum const value must be a u32 or smaller")]
    EnumConstTooLarge(u64),
    #[error("Missing Entry {0}")]
    MissingEntry(String),
    #[error("Missing Spec")]
    MissingSpec,
    #[error(transparent)]
    Xdr(XdrError),
    #[error(transparent)]
    Serde(serde_json::Error),

    #[error("Missing key {0} in map")]
    MissingKey(String),
    #[error("Failed to convert {0} to number")]
    FailedNumConversion(serde_json::Number),
    #[error("First argument in an enum must be a sybmol")]
    EnumFirstValueNotSymbol,
    #[error("Failed to find enum case {0}")]
    FailedToFindEnumCase(String),
}

impl From<()> for Error {
    fn from(_: ()) -> Self {
        Error::Unknown
    }
}

#[derive(Default, Clone)]
pub struct Spec(pub Option<Vec<ScSpecEntry>>);

impl Spec {
    /// # Errors
    /// Could fail to find User Defined Type
    pub fn doc(&self, name: &str, type_: &ScType) -> Result<Option<&'static str>, Error> {
        let mut str = match type_ {
            ScType::Val
            | ScType::U64
            | ScType::I64
            | ScType::U128
            | ScType::I128
            | ScType::U32
            | ScType::I32
            | ScType::Result(_)
            | ScType::Vec(_)
            | ScType::Map(_)
            | ScType::Set(_)
            | ScType::Tuple(_)
            | ScType::BytesN(_)
            | ScType::Symbol
            | ScType::Bitset
            | ScType::Status
            | ScType::Bytes
            | ScType::Address
            | ScType::Bool => String::new(),
            ScType::Option(type_) => return self.doc(name, &type_.value_type),
            ScType::Udt(ScSpecTypeUdt { name }) => {
                let spec_type = self.find(&name.to_string_lossy())?;
                match spec_type {
                    ScSpecEntry::FunctionV0(ScSpecFunctionV0 { doc, .. })
                    | ScSpecEntry::UdtStructV0(ScSpecUdtStructV0 { doc, .. })
                    | ScSpecEntry::UdtUnionV0(ScSpecUdtUnionV0 { doc, .. })
                    | ScSpecEntry::UdtEnumV0(ScSpecUdtEnumV0 { doc, .. })
                    | ScSpecEntry::UdtErrorEnumV0(ScSpecUdtErrorEnumV0 { doc, .. }) => doc,
                }
                .to_string_lossy()
            }
        };
        if let Some(mut ex) = self.example(type_) {
            if ex.contains(' ') {
                ex = format!("'{ex}'");
            } else if ex.contains('"') {
                ex = ex.replace('"', "");
            }
            if matches!(type_, ScType::Bool) {
                ex = String::new();
            }
            let sep = if str.is_empty() { "" } else { "\n" };
            str = format!("{str}{sep}Example:\n  --{name} {ex}");
            if ex.contains('"') {}
        }
        if str.is_empty() {
            Ok(None)
        } else {
            Ok(Some(Box::leak(str.into_boxed_str())))
        }
    }

    /// # Errors
    ///
    /// Might return errors
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

    /// # Errors
    ///
    /// Might return errors
    pub fn find_function(&self, name: &str) -> Result<&ScSpecFunctionV0, Error> {
        match self.find(name)? {
            ScSpecEntry::FunctionV0(f) => Ok(f),
            _ => Err(Error::MissingEntry(name.to_owned())),
        }
    }
    //
    /// # Errors
    ///
    pub fn find_functions(&self) -> Result<impl Iterator<Item = &ScSpecFunctionV0>, Error> {
        Ok(self
            .0
            .as_ref()
            .ok_or(Error::MissingSpec)?
            .iter()
            .filter_map(|e| match e {
                ScSpecEntry::FunctionV0(x) => Some(x),
                _ => None,
            }))
    }

    /// # Errors
    ///
    /// Might return errors
    pub fn from_string_primitive(s: &str, t: &ScType) -> Result<ScVal, Error> {
        Self::default().from_string(s, t)
    }

    /// # Errors
    ///
    /// Might return errors
    #[allow(clippy::wrong_self_convention)]
    pub fn from_string(&self, s: &str, t: &ScType) -> Result<ScVal, Error> {
        if let ScType::Option(b) = t {
            if s == "null" {
                return Ok(ScVal::Static(ScStatic::Void));
            }
            let ScSpecTypeOption { value_type } = b.as_ref().clone();
            let v = value_type.as_ref().clone();
            return self.from_string(s, &v);
        }
        // Parse as string and for special types assume Value::String
        serde_json::from_str(s)
            .map_or_else(
                |e| match t {
                    ScType::Symbol
                    | ScType::Bytes
                    | ScType::BytesN(_)
                    | ScType::U128
                    | ScType::I128
                    | ScType::Address => Ok(Value::String(s.to_owned())),
                    ScType::Udt(ScSpecTypeUdt { name })
                        if matches!(
                            self.find(&name.to_string_lossy())?,
                            ScSpecEntry::UdtUnionV0(_) | ScSpecEntry::UdtStructV0(_)
                        ) =>
                    {
                        Ok(Value::String(s.to_owned()))
                    }
                    _ => Err(Error::Serde(e)),
                },
                |val| match t {
                    ScType::U128 | ScType::I128 => Ok(Value::String(s.to_owned())),
                    _ => Ok(val),
                },
            )
            .and_then(|raw| self.from_json(&raw, t))
    }

    /// # Errors
    ///
    /// Might return errors
    #[allow(clippy::wrong_self_convention)]
    pub fn from_json(&self, v: &Value, t: &ScType) -> Result<ScVal, Error> {
        let val: ScVal = match (t, v) {
            (
                ScType::Bool
                | ScType::U128
                | ScType::I128
                | ScType::I32
                | ScType::I64
                | ScType::U32
                | ScType::U64
                | ScType::Symbol
                | ScType::Address
                | ScType::Bytes
                | ScType::BytesN(_),
                _,
            ) => from_json_primitives(v, t)?,

            // Vec parsing
            (ScType::Vec(elem), Value::Array(raw)) => {
                let converted: ScVec = raw
                    .iter()
                    .map(|item| self.from_json(item, &elem.element_type))
                    .collect::<Result<Vec<ScVal>, Error>>()?
                    .try_into()
                    .map_err(Error::Xdr)?;
                ScVal::Object(Some(ScObject::Vec(converted)))
            }

            // Map parsing
            (ScType::Map(map), Value::Object(raw)) => self.parse_map(map, raw)?,

            // Option parsing
            (ScType::Option(_), Value::Null) => ScVal::Static(ScStatic::Void),
            (ScType::Option(elem), v) => ScVal::Object(Some(
                self.from_json(v, &elem.value_type)?
                    .try_into()
                    .map_err(|_| Error::InvalidValue(Some(t.clone())))?,
            )),

            // Tuple parsing
            (ScType::Tuple(elem), Value::Array(raw)) => self.parse_tuple(t, elem, raw)?,

            (ScType::Udt(ScSpecTypeUdt { name }), _) => self.parse_udt(name, v)?,

            // TODO: Implement the rest of these
            // ScType::Bitset => {},
            // ScType::Status => {},
            // ScType::Result(Box<ScSpecTypeResult>) => {},
            (ScType::Set(set), Value::Array(values)) => self.parse_set(set, values)?,
            // ScType::Udt(ScSpecTypeUdt) => {},
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
                let v = map
                    .get(name)
                    .ok_or_else(|| Error::MissingKey(name.clone()))?;
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

        let case = union
            .cases
            .iter()
            .find(|c| {
                let name = match c {
                    ScSpecUdtUnionCaseV0::VoidV0(v) => &v.name,
                    ScSpecUdtUnionCaseV0::TupleV0(v) => &v.name,
                };
                enum_case == &name.to_string_lossy()
            })
            .ok_or_else(|| Error::EnumCase(enum_case.to_string(), union.name.to_string_lossy()))?;

        let s_vec = if let Some(value) = kind {
            let type_ = match case {
                ScSpecUdtUnionCaseV0::VoidV0(_) => todo!(),
                ScSpecUdtUnionCaseV0::TupleV0(v) => &v.type_[0],
            };
            let val = self.from_json(value, type_)?;
            let key = ScVal::Symbol(StringM::from_str(enum_case).map_err(Error::Xdr)?);
            vec![key, val]
        } else {
            let val = ScVal::Symbol(enum_case.try_into().map_err(Error::Xdr)?);
            vec![val]
        };

        Ok(ScVal::Object(Some(ScObject::Vec(
            s_vec.try_into().map_err(Error::Xdr)?,
        ))))
    }

    fn parse_tuple(
        &self,
        t: &ScType,
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

    fn parse_set(&self, set: &ScSpecTypeSet, values: &[Value]) -> Result<ScVal, Error> {
        let ScSpecTypeSet { element_type } = set;
        let parsed: Result<Vec<_>, Error> = values
            .iter()
            .map(|v| {
                let key = self.from_json(v, element_type)?;
                let val = ScVal::Static(ScStatic::Void);
                Ok(ScMapEntry { key, val })
            })
            .collect();
        Ok(ScVal::Object(Some(ScObject::Map(
            ScMap::sorted_from(parsed?).map_err(Error::Xdr)?,
        ))))
    }
}

impl Spec {
    /// # Errors
    ///
    /// Might return `Error::InvalidValue`
    ///
    /// # Panics
    ///
    /// May panic
    pub fn xdr_to_json(&self, res: &ScVal, output: &ScType) -> Result<Value, Error> {
        Ok(match (res, output) {
            (ScVal::Static(v), _) => match v {
                ScStatic::True => Value::Bool(true),
                ScStatic::False => Value::Bool(false),
                ScStatic::Void => Value::Null,
                ScStatic::LedgerKeyContractCode => return Err(Error::InvalidValue(None)),
            },
            (ScVal::U63(v), _) => Value::Number(serde_json::Number::from(*v)),
            (ScVal::U32(v), _) => Value::Number(serde_json::Number::from(*v)),
            (ScVal::I32(v), _) => Value::Number(serde_json::Number::from(*v)),
            (ScVal::Symbol(v), _) => Value::String(
                std::str::from_utf8(v.as_slice())
                    .map_err(|_| Error::InvalidValue(Some(ScType::Symbol)))?
                    .to_string(),
            ),

            (ScVal::Object(None), ScType::Option(_)) => Value::Null,
            (ScVal::Object(Some(inner)), ScType::Option(type_)) => {
                self.sc_object_to_json(inner, &type_.value_type)?
            }
            (ScVal::Object(Some(inner)), type_) => self.sc_object_to_json(inner, type_)?,

            (ScVal::Bitset(_), ScType::Bitset) => todo!(),

            (ScVal::Status(_), ScType::Status) => todo!(),
            (v, typed) => todo!("{v:#?} doesn't have a matching {typed:#?}"),
        })
    }

    /// # Errors
    ///
    /// Might return an error
    pub fn vec_m_to_json(
        &self,
        vec_m: &VecM<ScVal, 256_000>,
        type_: &ScType,
    ) -> Result<Value, Error> {
        Ok(Value::Array(
            vec_m
                .to_vec()
                .iter()
                .map(|sc_val| self.xdr_to_json(sc_val, type_))
                .collect::<Result<Vec<_>, Error>>()?,
        ))
    }

    /// # Errors
    ///
    /// Might return an error
    pub fn sc_map_to_json(&self, sc_map: &ScMap, type_: &ScSpecTypeMap) -> Result<Value, Error> {
        let v = sc_map
            .iter()
            .map(|ScMapEntry { key, val }| {
                let key_s = self.xdr_to_json(key, &type_.key_type)?.to_string();
                let val_value = self.xdr_to_json(val, &type_.value_type)?;
                Ok((key_s, val_value))
            })
            .collect::<Result<serde_json::Map<String, Value>, Error>>()?;
        Ok(Value::Object(v))
    }

    /// # Errors
    ///
    /// Might return an error
    pub fn sc_set_to_json(&self, sc_map: &ScMap, type_: &ScSpecTypeSet) -> Result<Value, Error> {
        let v = sc_map
            .iter()
            .map(|ScMapEntry { key, .. }| self.xdr_to_json(key, &type_.element_type))
            .collect::<Result<Vec<Value>, Error>>()?;
        Ok(Value::Array(v))
    }

    /// # Errors
    ///
    /// Might return an error
    ///
    /// # Panics
    ///
    /// May panic
    pub fn udt_to_json(&self, name: &StringM<60>, sc_obj: &ScObject) -> Result<Value, Error> {
        let name = &name.to_string_lossy();
        let udt = self.find(name)?;
        Ok(match (udt, sc_obj) {
            (ScSpecEntry::UdtStructV0(strukt), ScObject::Map(map)) => serde_json::Value::Object(
                strukt
                    .fields
                    .iter()
                    .zip(map.iter())
                    .map(|(field, entry)| {
                        let val = self.xdr_to_json(&entry.val, &field.type_)?;
                        Ok((field.name.to_string_lossy(), val))
                    })
                    .collect::<Result<serde_json::Map<String, _>, Error>>()?,
            ),
            (ScSpecEntry::UdtStructV0(strukt), ScObject::Vec(vec_)) => Value::Array(
                strukt
                    .fields
                    .iter()
                    .zip(vec_.iter())
                    .map(|(field, entry)| self.xdr_to_json(entry, &field.type_))
                    .collect::<Result<Vec<_>, Error>>()?,
            ),
            (ScSpecEntry::UdtUnionV0(union), ScObject::Vec(vec_)) => {
                let v = vec_.to_vec();
                let val = &v[0];
                let second_val = v.get(1);

                let ScVal::Symbol(case_name) = val else {
                     return Err(Error::EnumFirstValueNotSymbol)
                };
                let case = union
                    .cases
                    .iter()
                    .find(|case| {
                        let name = match case {
                            ScSpecUdtUnionCaseV0::VoidV0(v) => &v.name,
                            ScSpecUdtUnionCaseV0::TupleV0(v) => &v.name,
                        };
                        name.as_vec() == case_name.as_vec()
                    })
                    .ok_or_else(|| Error::FailedToFindEnumCase(case_name.to_string_lossy()))?;

                let case_name = case_name.to_string_lossy();
                match case {
                    ScSpecUdtUnionCaseV0::TupleV0(v) => {
                        let type_ = &v.type_[0];
                        let second_val = second_val.ok_or_else(|| {
                            Error::EnumMissingSecondValue(
                                case_name.clone(),
                                type_.name().to_string(),
                            )
                        })?;
                        let map: serde_json::Map<String, _> =
                            [(case_name, self.xdr_to_json(second_val, type_)?)]
                                .into_iter()
                                .collect();
                        Value::Object(map)
                    }
                    ScSpecUdtUnionCaseV0::VoidV0(_) => Value::String(case_name),
                }
            }
            (ScSpecEntry::UdtEnumV0(_enum_), _) => todo!(),
            (s, v) => todo!("Not implemented for {s:#?} {v:#?}"),
        })
    }

    /// # Errors
    ///
    /// Might return an error
    ///
    /// # Panics
    ///
    /// Some types are not yet supported and will cause a panic if supplied
    pub fn sc_object_to_json(&self, obj: &ScObject, spec_type: &ScType) -> Result<Value, Error> {
        Ok(match (obj, spec_type) {
            (ScObject::Vec(ScVec(vec_m)), ScType::Vec(type_)) => {
                self.vec_m_to_json(vec_m, &type_.element_type)?
            }
            (ScObject::Vec(ScVec(vec_m)), ScType::Tuple(tuple_type)) => Value::Array(
                vec_m
                    .iter()
                    .zip(tuple_type.value_types.iter())
                    .map(|(v, t)| self.xdr_to_json(v, t))
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            // (ScObject::Vec(_), ScType::Map(_)) => todo!(),
            // (ScObject::Vec(_), ScType::Set(_)) => todo!(),
            // (ScObject::Vec(_), ScType::BytesN(_)) => todo!(),
            (
                sc_obj @ (ScObject::Vec(_) | ScObject::Map(_)),
                ScType::Udt(ScSpecTypeUdt { name }),
            ) => self.udt_to_json(name, sc_obj)?,

            (ScObject::Map(map), ScType::Map(map_type)) => self.sc_map_to_json(map, map_type)?,

            (ScObject::Map(map), ScType::Set(set_type)) => self.sc_set_to_json(map, set_type)?,

            (ScObject::U64(u64_), ScType::U64) => Value::Number(serde_json::Number::from(*u64_)),

            (ScObject::I64(i64_), ScType::I64) => Value::Number(serde_json::Number::from(*i64_)),
            (int @ ScObject::U128(_), ScType::U128) => {
                // Always output u128s as strings
                let v: u128 = int
                    .clone()
                    .try_into()
                    .map_err(|_| Error::InvalidValue(Some(ScType::U128)))?;
                Value::String(v.to_string())
            }

            (int @ ScObject::I128(_), ScType::I128) => {
                // Always output u128s as strings
                let v: i128 = int
                    .clone()
                    .try_into()
                    .map_err(|_| Error::InvalidValue(Some(ScType::I128)))?;
                Value::String(v.to_string())
            }

            (ScObject::Bytes(v), ScType::Bytes | ScType::BytesN(_)) => {
                Value::String(to_lower_hex(v.as_slice()))
            }

            (ScObject::Bytes(_), ScType::Udt(_)) => todo!(),

            (ScObject::ContractCode(_), _) => todo!(),

            (ScObject::Address(v), ScType::Address) => sc_address_to_json(v),

            _ => return Err(Error::Unknown),
        })
    }
}

/// # Errors
///
/// Might return an error
pub fn from_string_primitive(s: &str, t: &ScType) -> Result<ScVal, Error> {
    Spec::from_string_primitive(s, t)
}

fn parse_const_enum(num: &serde_json::Number, enum_: &ScSpecUdtEnumV0) -> Result<ScVal, Error> {
    let num = num
        .as_u64()
        .ok_or_else(|| Error::FailedNumConversion(num.clone()))?;
    let num = u32::try_from(num).map_err(|_| Error::EnumConstTooLarge(num))?;
    enum_
        .cases
        .iter()
        .find(|c| c.value == num)
        .ok_or(Error::EnumConst(num))
        .map(|c| ScVal::U32(c.value))
}

/// # Errors
///
/// Might return an error
pub fn from_json_primitives(v: &Value, t: &ScType) -> Result<ScVal, Error> {
    let val: ScVal = match (t, v) {
        // Boolean parsing
        (ScType::Bool, Value::Bool(true)) => ScVal::Static(ScStatic::True),
        (ScType::Bool, Value::Bool(false)) => ScVal::Static(ScStatic::False),

        // Number parsing
        (ScType::U128, Value::String(s)) => {
            let val: u128 = u128::from_str(s)
                .map(Into::into)
                .map_err(|_| Error::InvalidValue(Some(t.clone())))?;
            ScVal::Object(Some(val.into()))
        }

        (ScType::I128, Value::String(s)) => {
            let val: i128 = i128::from_str(s)
                .map(Into::into)
                .map_err(|_| Error::InvalidValue(Some(t.clone())))?;
            ScVal::Object(Some(val.into()))
        }

        (ScType::I32, Value::Number(n)) => ScVal::I32(
            n.as_i64()
                .ok_or_else(|| Error::InvalidValue(Some(t.clone())))?
                .try_into()
                .map_err(|_| Error::InvalidValue(Some(t.clone())))?,
        ),
        (ScType::I64, Value::Number(n)) => ScVal::Object(Some(ScObject::I64(
            n.as_i64()
                .ok_or_else(|| Error::InvalidValue(Some(t.clone())))?,
        ))),
        (ScType::U32, Value::Number(n)) => ScVal::U32(
            n.as_u64()
                .ok_or_else(|| Error::InvalidValue(Some(t.clone())))?
                .try_into()
                .map_err(|_| Error::InvalidValue(Some(t.clone())))?,
        ),
        (ScType::U64, Value::Number(n)) => ScVal::Object(Some(ScObject::U64(
            n.as_u64()
                .ok_or_else(|| Error::InvalidValue(Some(t.clone())))?,
        ))),

        // Symbol parsing
        (ScType::Symbol, Value::String(s)) => ScVal::Symbol(
            s.as_bytes()
                .try_into()
                .map_err(|_| Error::InvalidValue(Some(t.clone())))?,
        ),

        (ScType::Address, Value::String(s)) => sc_address_from_json(s, t)?,

        // Bytes parsing
        (ScType::BytesN(bytes), Value::String(s)) => ScVal::Object(Some(ScObject::Bytes({
            if let Ok(key) = stellar_strkey::ed25519::PublicKey::from_string(s) {
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
        (ScType::Bytes, Value::String(s)) => ScVal::Object(Some(ScObject::Bytes(
            hex::decode(s)
                .map_err(|_| Error::InvalidValue(Some(t.clone())))?
                .try_into()
                .map_err(|_| Error::InvalidValue(Some(t.clone())))?,
        ))),
        (ScType::Bytes | ScType::BytesN(_), Value::Array(raw)) => {
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

/// # Errors
///
/// Might return an error
pub fn to_string(v: &ScVal) -> Result<String, Error> {
    #[allow(clippy::match_same_arms)]
    Ok(match v {
        // If symbols are a top-level thing we omit the wrapping quotes
        // TODO: Decide if this is a good idea or not.
        ScVal::Symbol(v) => std::str::from_utf8(v.as_slice())
            .map_err(|_| Error::InvalidValue(Some(ScType::Symbol)))?
            .to_string(),
        _ => serde_json::to_string(&to_json(v)?).map_err(Error::Serde)?,
    })
}

/// # Errors
///
/// Might return an error
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
                .map_err(|_| Error::InvalidValue(Some(ScType::Symbol)))?
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
        ScVal::Object(Some(ScObject::Address(v))) => sc_address_to_json(v),
        ScVal::Object(Some(ScObject::NonceKey(v))) => sc_address_to_json(v),
        ScVal::Object(Some(ScObject::U128(n))) => {
            // Always output u128s as strings
            let v: u128 = ScObject::U128(n.clone())
                .try_into()
                .map_err(|_| Error::InvalidValue(Some(ScType::U128)))?;
            Value::String(v.to_string())
        }
        ScVal::Object(Some(ScObject::I128(n))) => {
            // Always output i128s as strings
            let v: i128 = ScObject::I128(n.clone())
                .try_into()
                .map_err(|_| Error::InvalidValue(Some(ScType::I128)))?;
            Value::String(v.to_string())
        }
        // TODO: Implement these
        ScVal::Object(Some(ScObject::ContractCode(_))) | ScVal::Bitset(_) | ScVal::Status(_) => {
            serde_json::to_value(v).map_err(Error::Serde)?
        }
    };
    Ok(val)
}

fn sc_address_to_json(v: &ScAddress) -> Value {
    match v {
        ScAddress::Account(AccountId(PublicKey::PublicKeyTypeEd25519(Uint256(k)))) => {
            Value::String(stellar_strkey::ed25519::PublicKey(*k).to_string())
        }
        ScAddress::Contract(Hash(h)) => Value::String(stellar_strkey::Contract(*h).to_string()),
    }
}

fn sc_address_from_json(s: &str, t: &ScType) -> Result<ScVal, Error> {
    stellar_strkey::Strkey::from_string(s)
        .map_err(|_| Error::InvalidValue(Some(t.clone())))
        .map(|parsed| match parsed {
            stellar_strkey::Strkey::PublicKeyEd25519(p) => {
                Some(ScVal::Object(Some(ScObject::Address(ScAddress::Account(
                    AccountId(PublicKey::PublicKeyTypeEd25519(Uint256(p.0))),
                )))))
            }
            stellar_strkey::Strkey::Contract(c) => Some(ScVal::Object(Some(ScObject::Address(
                ScAddress::Contract(Hash(c.0)),
            )))),
            _ => None,
        })?
        .ok_or(Error::InvalidValue(Some(t.clone())))
}

fn to_lower_hex(bytes: &[u8]) -> String {
    let mut res = String::with_capacity(bytes.len());
    for b in bytes {
        res.push_str(&format!("{b:02x}"));
    }
    res
}

impl Spec {
    #[must_use]
    pub fn arg_value_name(&self, type_: &ScType, depth: usize) -> Option<String> {
        match type_ {
            ScType::U64 => Some("u64".to_string()),
            ScType::I64 => Some("i64".to_string()),
            ScType::U128 => Some("u128".to_string()),
            ScType::I128 => Some("i128".to_string()),
            ScType::U32 => Some("u32".to_string()),
            ScType::I32 => Some("i32".to_string()),
            ScType::Bool => Some("bool".to_string()),
            ScType::Symbol => Some("Symbol".to_string()),
            ScType::Bitset => Some("Bitset".to_string()),
            ScType::Status => Some("Status".to_string()),
            ScType::Bytes => Some("hex_bytes".to_string()),
            ScType::Address => Some("Address".to_string()),
            ScType::Option(val) => {
                let ScSpecTypeOption { value_type } = val.as_ref();
                let inner = self.arg_value_name(value_type.as_ref(), depth + 1)?;
                Some(format!("Option<{inner}>"))
            }
            ScType::Vec(val) => {
                let ScSpecTypeVec { element_type } = val.as_ref();
                let inner = self.arg_value_name(element_type.as_ref(), depth + 1)?;
                Some(format!("Array<{inner}>"))
            }
            ScType::Set(val) => {
                let ScSpecTypeSet { element_type } = val.as_ref();
                let inner = self.arg_value_name(element_type.as_ref(), depth + 1)?;
                Some(format!("Set<{inner}>"))
            }
            ScType::Result(val) => {
                let ScSpecTypeResult {
                    ok_type,
                    error_type,
                } = val.as_ref();
                let ok = self.arg_value_name(ok_type.as_ref(), depth + 1)?;
                let error = self.arg_value_name(error_type.as_ref(), depth + 1)?;
                Some(format!("Result<{ok}, {error}>"))
            }
            ScType::Tuple(val) => {
                let ScSpecTypeTuple { value_types } = val.as_ref();
                let names = value_types
                    .iter()
                    .map(|t| self.arg_value_name(t, depth + 1))
                    .collect::<Option<Vec<_>>>()?
                    .join(", ");
                Some(format!("Tuple<{names}>"))
            }
            ScType::Map(val) => {
                let ScSpecTypeMap {
                    key_type,
                    value_type,
                } = val.as_ref();
                let (key, val) = (
                    self.arg_value_name(key_type.as_ref(), depth + 1)?,
                    self.arg_value_name(value_type.as_ref(), depth + 1)?,
                );
                Some(format!("Map<{key}, {val}>"))
            }
            ScType::BytesN(t) => Some(format!("{}_hex_bytes", t.n)),
            ScType::Udt(ScSpecTypeUdt { name }) => {
                match self.find(&name.to_string_lossy()).ok()? {
                    ScSpecEntry::UdtStructV0(strukt) => self.arg_value_udt(strukt, depth),
                    ScSpecEntry::UdtUnionV0(union) => self.arg_value_union(union, depth),
                    ScSpecEntry::UdtEnumV0(enum_) => Some(arg_value_enum(enum_)),
                    ScSpecEntry::FunctionV0(_) | ScSpecEntry::UdtErrorEnumV0(_) => None,
                }
            }
            // No specific value name for these yet.
            ScType::Val => None,
        }
    }

    fn arg_value_udt(&self, strukt: &ScSpecUdtStructV0, depth: usize) -> Option<String> {
        let inner = strukt
            .fields
            .iter()
            .map(|f| (f.name.to_string_lossy(), &f.type_))
            .map(|(name, type_)| {
                let type_ = self.arg_value_name(type_, depth + 1)?;
                Some(format!("{name}: {type_}"))
            })
            .collect::<Option<Vec<_>>>()?
            .join(", ");
        Some(format!("{{ {inner} }}"))
    }

    fn arg_value_union(&self, union: &ScSpecUdtUnionV0, depth: usize) -> Option<String> {
        union
            .cases
            .iter()
            .map(|f| {
                Some(match f {
                    xdr::ScSpecUdtUnionCaseV0::VoidV0(ScSpecUdtUnionCaseVoidV0 {
                        name, ..
                    }) => name.to_string_lossy(),
                    xdr::ScSpecUdtUnionCaseV0::TupleV0(ScSpecUdtUnionCaseTupleV0 {
                        name,
                        type_,
                        ..
                    }) => format!(
                        "{}({})",
                        name.to_string_lossy(),
                        type_
                            .iter()
                            .map(|type_| self.arg_value_name(type_, depth + 1))
                            .collect::<Option<Vec<String>>>()?
                            .join(",")
                    ),
                })
            })
            .collect::<Option<Vec<_>>>()
            .map(|v| v.join(" | "))
    }
}

fn arg_value_enum(enum_: &ScSpecUdtEnumV0) -> String {
    enum_
        .cases
        .iter()
        .map(|case| case.value.to_string())
        .join(" | ")
}

// Example implementation
impl Spec {
    #[must_use]
    pub fn example(&self, type_: &ScType) -> Option<String> {
        match type_ {
            ScType::U64 => Some("42".to_string()),
            ScType::I64 => Some("-42".to_string()),
            ScType::U128 => Some("\"1000\"".to_string()),
            ScType::I128 => Some("\"-100\"".to_string()),
            ScType::U32 => Some("1".to_string()),
            ScType::I32 => Some("-1".to_string()),
            ScType::Bool => Some("true".to_string()),
            ScType::Symbol => Some("\"hello\"".to_string()),
            ScType::Bitset => Some("Bitset".to_string()),
            ScType::Status => Some("Status".to_string()),
            ScType::Bytes => Some("\"beefface123\"".to_string()),
            ScType::Address => {
                Some("\"GDIY6AQQ75WMD4W46EYB7O6UYMHOCGQHLAQGQTKHDX4J2DYQCHVCR4W4\"".to_string())
            }
            ScType::Option(val) => {
                let ScSpecTypeOption { value_type } = val.as_ref();
                self.example(value_type.as_ref())
            }
            ScType::Vec(val) => {
                let ScSpecTypeVec { element_type } = val.as_ref();
                let inner = self.example(element_type.as_ref())?;
                Some(format!("[ {inner} ]"))
            }
            ScType::Set(val) => {
                let ScSpecTypeSet { element_type } = val.as_ref();
                let inner = self.example(element_type.as_ref())?;
                Some(format!("[ {inner} ]"))
            }
            ScType::Result(val) => {
                let ScSpecTypeResult {
                    ok_type,
                    error_type,
                } = val.as_ref();
                let ok = self.example(ok_type.as_ref())?;
                let error = self.example(error_type.as_ref())?;
                Some(format!("Result<{ok}, {error}>"))
            }
            ScType::Tuple(val) => {
                let ScSpecTypeTuple { value_types } = val.as_ref();
                let names = value_types
                    .iter()
                    .map(|t| self.example(t))
                    .collect::<Option<Vec<_>>>()?
                    .join(", ");
                Some(format!("[{names}]"))
            }
            ScType::Map(map) => {
                let ScSpecTypeMap {
                    key_type,
                    value_type,
                } = map.as_ref();
                let (mut key, val) = (
                    self.example(key_type.as_ref())?,
                    self.example(value_type.as_ref())?,
                );
                if !matches!(key_type.as_ref(), ScType::Symbol) {
                    key = format!("\"{key}\"");
                }
                Some(format!("{{ {key}: {val} }}"))
            }
            ScType::BytesN(n) => {
                let n = n.n as usize;
                let res = if n % 2 == 0 {
                    "ef".repeat(n)
                } else {
                    let mut s = "ef".repeat(n - 1);
                    s.push('e');
                    s
                };
                Some(format!("\"{res}\""))
            }
            ScType::Udt(ScSpecTypeUdt { name }) => match self.find(&name.to_string_lossy()).ok() {
                Some(ScSpecEntry::UdtStructV0(strukt)) => {
                    let inner = strukt
                        .fields
                        .iter()
                        .map(|f| (f.name.to_string_lossy(), &f.type_))
                        .map(|(name, type_)| {
                            let type_ = self.example(type_)?;
                            let name = format!(r#""{name}""#);
                            Some(format!("{name}: {type_}"))
                        })
                        .collect::<Option<Vec<_>>>()?
                        .join(", ");
                    Some(format!(r#"{{ {inner} }}"#))
                }
                Some(ScSpecEntry::UdtUnionV0(union)) => self.example_union(union),
                Some(ScSpecEntry::UdtEnumV0(enum_)) => {
                    enum_.cases.iter().next().map(|c| c.value.to_string())
                }
                Some(ScSpecEntry::FunctionV0(_) | ScSpecEntry::UdtErrorEnumV0(_)) | None => None,
            },
            // No specific value name for these yet.
            ScType::Val => None,
        }
    }

    fn example_union(&self, union: &ScSpecUdtUnionV0) -> Option<String> {
        let case = union.cases.iter().next()?;
        let res = match case {
            xdr::ScSpecUdtUnionCaseV0::VoidV0(ScSpecUdtUnionCaseVoidV0 { name, .. }) => {
                name.to_string_lossy()
            }
            xdr::ScSpecUdtUnionCaseV0::TupleV0(ScSpecUdtUnionCaseTupleV0 {
                name, type_, ..
            }) => {
                let names = type_
                    .iter()
                    .map(|t| self.example(t))
                    .collect::<Option<Vec<_>>>()?
                    .join(", ");
                format!("[\"{}\", {names}]", name.to_string_lossy())
            }
        };
        Some(res)
    }
}
