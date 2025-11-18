#![allow(clippy::missing_errors_doc, clippy::must_use_candidate)]
use std::fmt::Write;
use std::str::FromStr;

use itertools::Itertools;
use serde_json::{json, Value};
use stellar_xdr::curr::{
    AccountId, BytesM, ContractExecutable, ContractId, Error as XdrError, Hash, Int128Parts,
    Int256Parts, MuxedEd25519Account, PublicKey, ScAddress, ScBytes, ScContractInstance, ScMap,
    ScMapEntry, ScNonceKey, ScSpecEntry, ScSpecEventV0, ScSpecFunctionV0, ScSpecTypeDef as ScType,
    ScSpecTypeMap, ScSpecTypeOption, ScSpecTypeResult, ScSpecTypeTuple,
    ScSpecTypeUdt, ScSpecTypeVec, ScSpecUdtEnumV0, ScSpecUdtErrorEnumCaseV0, ScSpecUdtErrorEnumV0,
    ScSpecUdtStructV0, ScSpecUdtUnionCaseTupleV0, ScSpecUdtUnionCaseV0, ScSpecUdtUnionCaseVoidV0,
    ScSpecUdtUnionV0, ScString, ScSymbol, ScVal, ScVec, StringM, UInt128Parts, UInt256Parts,
    Uint256, VecM,
};

pub mod contract;
pub mod utils;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("an unknown error occurred")]
    Unknown,
    #[error("Invalid pair {0:#?} {1:#?}")]
    InvalidPair(ScVal, ScType),
    #[error("value is not parseable to {0:#?}")]
    InvalidValue(Option<ScType>),
    #[error("Unknown case {0} for {1}")]
    EnumCase(String, String),
    #[error("Enum {0} missing value for type {1}")]
    EnumMissingSecondValue(String, String),
    #[error("Enum {0} is illformed")]
    IllFormedEnum(String),
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
    Serde(#[from] serde_json::Error),
    #[error(transparent)]
    Ethnum(#[from] core::num::ParseIntError),

    #[error("Missing key {0} in map")]
    MissingKey(String),
    #[error("Failed to convert {0} to number")]
    FailedNumConversion(serde_json::Number),
    #[error("First argument in an enum must be a sybmol")]
    EnumFirstValueNotSymbol,
    #[error("Failed to find enum case {0}")]
    FailedToFindEnumCase(String),
    #[error(transparent)]
    FailedSilceToByte(#[from] std::array::TryFromSliceError),
    #[error(transparent)]
    Infallible(#[from] std::convert::Infallible),
    #[error("Missing Error case {0}")]
    MissingErrorCase(u32),
    #[error(transparent)]
    Spec(#[from] soroban_spec::read::FromWasmError),
    #[error(transparent)]
    Base64Spec(#[from] soroban_spec::read::ParseSpecBase64Error),
}

#[derive(Default, Clone)]
pub struct Spec(pub Option<Vec<ScSpecEntry>>);

impl TryInto<Spec> for &[u8] {
    type Error = soroban_spec::read::FromWasmError;

    fn try_into(self) -> Result<Spec, Self::Error> {
        let spec = soroban_spec::read::from_wasm(self)?;
        Ok(Spec::new(spec.as_slice()))
    }
}

impl Spec {
    pub fn new(entries: &[ScSpecEntry]) -> Self {
        Self(Some(entries.to_vec()))
    }

    pub fn from_wasm(wasm: &[u8]) -> Result<Spec, Error> {
        let spec = soroban_spec::read::from_wasm(wasm)?;
        Ok(Spec::new(spec.as_slice()))
    }

    pub fn parse_base64(base64: &str) -> Result<Spec, Error> {
        let spec = soroban_spec::read::parse_base64(base64.as_bytes())?;
        Ok(Spec::new(spec.as_slice()))
    }
}

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
            | ScType::Tuple(_)
            | ScType::BytesN(_)
            | ScType::Symbol
            | ScType::Error
            | ScType::Bytes
            | ScType::Void
            | ScType::Timepoint
            | ScType::Duration
            | ScType::U256
            | ScType::I256
            | ScType::String
            | ScType::Bool => String::new(),
            ScType::MuxedAddress => {
                String::from("Can be public key (G13..), contract ID (C13...), or a muxed account (M13..), or an identity")
            }
            ScType::Address => String::from(
                "Can be public key (G13..), a contract ID (C13...) or an identity (alice), ",
            ),
            ScType::Option(type_) => return self.doc(name, &type_.value_type),
            ScType::Udt(ScSpecTypeUdt { name }) => {
                let spec_type = self.find(&name.to_utf8_string_lossy())?;
                let spec_type_match = match spec_type {
                    ScSpecEntry::FunctionV0(ScSpecFunctionV0 { doc, .. })
                    | ScSpecEntry::UdtStructV0(ScSpecUdtStructV0 { doc, .. })
                    | ScSpecEntry::UdtUnionV0(ScSpecUdtUnionV0 { doc, .. })
                    | ScSpecEntry::UdtEnumV0(ScSpecUdtEnumV0 { doc, .. })
                    | ScSpecEntry::UdtErrorEnumV0(ScSpecUdtErrorEnumV0 { doc, .. })
                    | ScSpecEntry::EventV0(ScSpecEventV0 { doc, .. }) => doc,
                };
                spec_type_match.to_utf8_string_lossy()
            }
        };

        if let Some(mut ex) = self.example(0, type_) {
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
                        ScSpecEntry::FunctionV0(x) => x.name.to_utf8_string_lossy(),
                        ScSpecEntry::UdtStructV0(x) => x.name.to_utf8_string_lossy(),
                        ScSpecEntry::UdtUnionV0(x) => x.name.to_utf8_string_lossy(),
                        ScSpecEntry::UdtEnumV0(x) => x.name.to_utf8_string_lossy(),
                        ScSpecEntry::UdtErrorEnumV0(x) => x.name.to_utf8_string_lossy(),
                        ScSpecEntry::EventV0(x) => x.name.to_utf8_string_lossy(),
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
            .as_deref()
            .ok_or(Error::MissingSpec)?
            .iter()
            .filter_map(|e| match e {
                ScSpecEntry::FunctionV0(x) => Some(x),
                _ => None,
            }))
    }

    /// # Errors
    ///
    pub fn find_error_type(&self, value: u32) -> Result<&ScSpecUdtErrorEnumCaseV0, Error> {
        if let ScSpecEntry::UdtErrorEnumV0(ScSpecUdtErrorEnumV0 { cases, .. }) =
            self.find("Error")?
        {
            if let Some(case) = cases.iter().find(|case| value == case.value) {
                return Ok(case);
            }
        }
        Err(Error::MissingErrorCase(value))
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
                return Ok(ScVal::Void);
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
                    | ScType::String
                    | ScType::Bytes
                    | ScType::BytesN(_)
                    | ScType::U256
                    | ScType::I256
                    | ScType::U128
                    | ScType::I128
                    | ScType::Address
                    | ScType::MuxedAddress => Ok(Value::String(s.to_owned())),
                    ScType::Udt(ScSpecTypeUdt { name })
                        if matches!(
                            self.find(&name.to_utf8_string_lossy())?,
                            ScSpecEntry::UdtUnionV0(_) | ScSpecEntry::UdtStructV0(_)
                        ) =>
                    {
                        Ok(Value::String(s.to_owned()))
                    }
                    _ => Err(Error::Serde(e)),
                },
                |val| match t {
                    ScType::U128 | ScType::I128 | ScType::U256 | ScType::I256 => {
                        Ok(Value::String(s.to_owned()))
                    }
                    ScType::Timepoint | ScType::Duration => {
                        // timepoint and duration both expect a JSON object with the value
                        // being the u64 number as a string, and key being the type name
                        let key = match t {
                            ScType::Timepoint => "timepoint",
                            ScType::Duration => "duration",
                            _ => unreachable!(),
                        };

                        Ok(json!({ key: s }))
                    }
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
                | ScType::U256
                | ScType::I256
                | ScType::I32
                | ScType::I64
                | ScType::U32
                | ScType::U64
                | ScType::String
                | ScType::Symbol
                | ScType::Address
                | ScType::MuxedAddress
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
                ScVal::Vec(Some(converted))
            }

            // Map parsing
            (ScType::Map(map), Value::Object(raw)) => self.parse_map(map, raw)?,

            // Option parsing
            (ScType::Option(_), Value::Null) => ScVal::Void,
            (ScType::Option(elem), v) => self.from_json(v, &elem.value_type)?,

            // Tuple parsing
            (ScType::Tuple(elem), Value::Array(raw)) => self.parse_tuple(t, elem, raw)?,

            // User defined types parsing
            (ScType::Udt(ScSpecTypeUdt { name }), _) => self.parse_udt(name, v)?,

            // TODO: Implement the rest of these
            (_, raw) => serde_json::from_value(raw.clone()).map_err(Error::Serde)?,
        };
        Ok(val)
    }

    fn parse_udt(&self, name: &StringM<60>, value: &Value) -> Result<ScVal, Error> {
        let name = &name.to_utf8_string_lossy();
        match (self.find(name)?, value) {
            (ScSpecEntry::UdtStructV0(strukt), Value::Object(map)) => {
                if strukt
                    .fields
                    .iter()
                    .any(|f| f.name.to_utf8_string_lossy() == "0")
                {
                    self.parse_tuple_strukt(
                        strukt,
                        &(0..map.len())
                            .map(|i| map.get(&i.to_string()).unwrap().clone())
                            .collect::<Vec<_>>(),
                    )
                } else {
                    self.parse_strukt(strukt, map)
                }
            }
            (ScSpecEntry::UdtStructV0(strukt), Value::Array(arr)) => {
                self.parse_tuple_strukt(strukt, arr)
            }
            (
                ScSpecEntry::UdtUnionV0(union),
                val @ (Value::Array(_) | Value::String(_) | Value::Object(_)),
            ) => self.parse_union(union, val),
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
        Ok(ScVal::Vec(Some(items.try_into().map_err(Error::Xdr)?)))
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
                let name = &f.name.to_utf8_string_lossy();
                let v = map
                    .get(name)
                    .ok_or_else(|| Error::MissingKey(name.clone()))?;
                let val = self.from_json(v, &f.type_)?;
                let key = StringM::from_str(name).unwrap();
                Ok(ScMapEntry {
                    key: ScVal::Symbol(key.into()),
                    val,
                })
            })
            .collect::<Result<Vec<_>, Error>>()?;
        let map = ScMap::sorted_from(items).map_err(Error::Xdr)?;
        Ok(ScVal::Map(Some(map)))
    }

    fn parse_union(&self, union: &ScSpecUdtUnionV0, value: &Value) -> Result<ScVal, Error> {
        let (enum_case, rest) = match value {
            Value::String(s) => (s, None),
            Value::Object(o) if o.len() == 1 => {
                let res = o.values().next().map(|v| match v {
                    Value::Object(obj) if obj.contains_key("0") => {
                        let len = obj.len();
                        Value::Array(
                            (0..len)
                                .map(|i| obj.get(&i.to_string()).unwrap().clone())
                                .collect::<Vec<_>>(),
                        )
                    }
                    _ => v.clone(),
                });
                (o.keys().next().unwrap(), res)
            }
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
                enum_case == &name.to_utf8_string_lossy()
            })
            .ok_or_else(|| Error::EnumCase(enum_case.clone(), union.name.to_utf8_string_lossy()))?;

        let mut res = vec![ScVal::Symbol(ScSymbol(
            enum_case.try_into().map_err(Error::Xdr)?,
        ))];

        match (case, rest) {
            (ScSpecUdtUnionCaseV0::VoidV0(_), _) | (ScSpecUdtUnionCaseV0::TupleV0(_), None) => (),
            (ScSpecUdtUnionCaseV0::TupleV0(ScSpecUdtUnionCaseTupleV0 { type_, .. }), Some(arr))
                if type_.len() == 1 =>
            {
                res.push(self.from_json(&arr, &type_[0])?);
            }
            (
                ScSpecUdtUnionCaseV0::TupleV0(ScSpecUdtUnionCaseTupleV0 { type_, .. }),
                Some(Value::Array(arr)),
            ) => {
                res.extend(
                    arr.iter()
                        .zip(type_.iter())
                        .map(|(elem, ty)| self.from_json(elem, ty))
                        .collect::<Result<Vec<_>, _>>()?,
                );
            }
            (ScSpecUdtUnionCaseV0::TupleV0(ScSpecUdtUnionCaseTupleV0 { .. }), Some(_)) => {}
        }
        Ok(ScVal::Vec(Some(res.try_into().map_err(Error::Xdr)?)))
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
        }
        let parsed: Result<Vec<ScVal>, Error> = items
            .iter()
            .zip(value_types.iter())
            .map(|(item, t)| self.from_json(item, t))
            .collect();
        let converted: ScVec = parsed?.try_into().map_err(Error::Xdr)?;
        Ok(ScVal::Vec(Some(converted)))
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
        Ok(ScVal::Map(Some(
            ScMap::sorted_from(parsed?).map_err(Error::Xdr)?,
        )))
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
    pub fn xdr_to_json(&self, val: &ScVal, output: &ScType) -> Result<Value, Error> {
        Ok(match (val, output) {
            (ScVal::Void, ScType::Val | ScType::Option(_) | ScType::Tuple(_))
            | (ScVal::Map(None) | ScVal::Vec(None), ScType::Option(_)) => Value::Null,
            (ScVal::Bool(_), ScType::Bool)
            | (ScVal::Void, ScType::Void)
            | (ScVal::String(_), ScType::String)
            | (ScVal::Symbol(_), ScType::Symbol)
            | (ScVal::U64(_), ScType::U64)
            | (ScVal::I64(_), ScType::I64)
            | (ScVal::U32(_), ScType::U32)
            | (ScVal::I32(_), ScType::I32)
            | (ScVal::U128(_), ScType::U128)
            | (ScVal::I128(_), ScType::I128)
            | (ScVal::U256(_), ScType::U256)
            | (ScVal::I256(_), ScType::I256)
            | (ScVal::Duration(_), ScType::Duration)
            | (ScVal::Timepoint(_), ScType::Timepoint)
            | (
                ScVal::ContractInstance(_)
                | ScVal::LedgerKeyContractInstance
                | ScVal::LedgerKeyNonce(_),
                _,
            )
            | (ScVal::Address(_), ScType::Address | ScType::MuxedAddress)
            | (ScVal::Bytes(_), ScType::Bytes | ScType::BytesN(_)) => to_json(val)?,

            (val, ScType::Result(inner)) => self.xdr_to_json(val, &inner.ok_type)?,

            (val, ScType::Option(inner)) => self.xdr_to_json(val, &inner.value_type)?,
            (ScVal::Map(Some(_)) | ScVal::Vec(Some(_)) | ScVal::U32(_), type_) => {
                self.sc_object_to_json(val, type_)?
            }

            (ScVal::Error(_), ScType::Error) => todo!(),
            (v, typed) => todo!("{v:#?} doesn't have a matching {typed:#?}"),
        })
    }

    /// # Errors
    ///
    /// Might return an error
    pub fn vec_m_to_json<const MAX: u32>(
        &self,
        vec_m: &VecM<ScVal, MAX>,
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
    ///
    /// # Panics
    ///
    /// May panic
    pub fn udt_to_json(&self, name: &StringM<60>, sc_obj: &ScVal) -> Result<Value, Error> {
        let name = &name.to_utf8_string_lossy();
        let udt = self.find(name)?;
        Ok(match (sc_obj, udt) {
            (ScVal::Map(Some(map)), ScSpecEntry::UdtStructV0(strukt)) => serde_json::Value::Object(
                strukt
                    .fields
                    .iter()
                    .zip(map.iter())
                    .map(|(field, entry)| {
                        let val = self.xdr_to_json(&entry.val, &field.type_)?;
                        Ok((field.name.to_utf8_string_lossy(), val))
                    })
                    .collect::<Result<serde_json::Map<String, _>, Error>>()?,
            ),
            (ScVal::Vec(Some(vec_)), ScSpecEntry::UdtStructV0(strukt)) => Value::Array(
                strukt
                    .fields
                    .iter()
                    .zip(vec_.iter())
                    .map(|(field, entry)| self.xdr_to_json(entry, &field.type_))
                    .collect::<Result<Vec<_>, Error>>()?,
            ),
            (ScVal::Vec(Some(vec_)), ScSpecEntry::UdtUnionV0(union)) => {
                let v = vec_.to_vec();
                // let val = &v[0];
                let (first, rest) = match v.split_at(1) {
                    ([first], []) => (first, None),
                    ([first], rest) => (first, Some(rest)),
                    _ => return Err(Error::IllFormedEnum(union.name.to_utf8_string_lossy())),
                };

                let ScVal::Symbol(case_name) = first else {
                    return Err(Error::EnumFirstValueNotSymbol);
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
                    .ok_or_else(|| Error::FailedToFindEnumCase(case_name.to_utf8_string_lossy()))?;

                let case_name = case_name.to_utf8_string_lossy();
                match case {
                    ScSpecUdtUnionCaseV0::TupleV0(v) => {
                        let rest = rest.ok_or_else(|| {
                            Error::EnumMissingSecondValue(
                                union.name.to_utf8_string_lossy(),
                                case_name.clone(),
                            )
                        })?;
                        let val = if v.type_.len() == 1 {
                            self.xdr_to_json(&rest[0], &v.type_[0])?
                        } else {
                            Value::Array(
                                v.type_
                                    .iter()
                                    .zip(rest.iter())
                                    .map(|(type_, val)| self.xdr_to_json(val, type_))
                                    .collect::<Result<Vec<_>, Error>>()?,
                            )
                        };

                        Value::Object([(case_name, val)].into_iter().collect())
                    }
                    ScSpecUdtUnionCaseV0::VoidV0(_) => Value::String(case_name),
                }
            }
            (ScVal::U32(v), ScSpecEntry::UdtEnumV0(_enum_)) => {
                Value::Number(serde_json::Number::from(*v))
            }
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
    pub fn sc_object_to_json(&self, val: &ScVal, spec_type: &ScType) -> Result<Value, Error> {
        Ok(match (val, spec_type) {
            (ScVal::Vec(Some(ScVec(vec_m))), ScType::Vec(type_)) => {
                self.vec_m_to_json(vec_m, &type_.element_type)?
            }
            (ScVal::Vec(Some(ScVec(vec_m))), ScType::Tuple(tuple_type)) => Value::Array(
                vec_m
                    .iter()
                    .zip(tuple_type.value_types.iter())
                    .map(|(v, t)| self.xdr_to_json(v, t))
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            (
                sc_obj @ (ScVal::Vec(_) | ScVal::Map(_) | ScVal::U32(_)),
                ScType::Udt(ScSpecTypeUdt { name }),
            ) => self.udt_to_json(name, sc_obj)?,

            (ScVal::Map(Some(map)), ScType::Map(map_type)) => self.sc_map_to_json(map, map_type)?,

            (ScVal::U64(u64_), ScType::U64) => Value::Number(serde_json::Number::from(*u64_)),

            (ScVal::I64(i64_), ScType::I64) => Value::Number(serde_json::Number::from(*i64_)),
            (int @ ScVal::U128(_), ScType::U128) => {
                // Always output u128s as strings
                let v: u128 = int
                    .clone()
                    .try_into()
                    .map_err(|()| Error::InvalidValue(Some(ScType::U128)))?;
                Value::String(v.to_string())
            }

            (int @ ScVal::I128(_), ScType::I128) => {
                // Always output u128s as strings
                let v: i128 = int
                    .clone()
                    .try_into()
                    .map_err(|()| Error::InvalidValue(Some(ScType::I128)))?;
                Value::String(v.to_string())
            }

            (ScVal::Bytes(v), ScType::Bytes | ScType::BytesN(_)) => {
                Value::String(to_lower_hex(v.as_slice()))
            }

            (ScVal::Bytes(_), ScType::Udt(_)) => todo!(),

            (ScVal::ContractInstance(_), _) => todo!(),

            (ScVal::Address(v), ScType::Address | ScType::MuxedAddress) => sc_address_to_json(v),

            (ok_val, ScType::Result(result_type)) => {
                let ScSpecTypeResult { ok_type, .. } = result_type.as_ref();
                self.xdr_to_json(ok_val, ok_type)?
            }

            (x, y) => return Err(Error::InvalidPair(x.clone(), y.clone())),
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
#[allow(clippy::too_many_lines)]
pub fn from_json_primitives(v: &Value, t: &ScType) -> Result<ScVal, Error> {
    let val: ScVal = match (t, v) {
        // Boolean parsing
        (ScType::Bool, Value::Bool(true)) => ScVal::Bool(true),
        (ScType::Bool, Value::Bool(false)) => ScVal::Bool(false),

        // Number parsing
        (ScType::U128, Value::String(s)) => {
            let val: u128 = u128::from_str(s).map_err(|_| Error::InvalidValue(Some(t.clone())))?;
            let bytes = val.to_be_bytes();
            let (hi, lo) = bytes.split_at(8);
            ScVal::U128(UInt128Parts {
                hi: u64::from_be_bytes(hi.try_into()?),
                lo: u64::from_be_bytes(lo.try_into()?),
            })
        }

        (ScType::I128, Value::String(s)) => {
            let val: i128 = i128::from_str(s).map_err(|_| Error::InvalidValue(Some(t.clone())))?;
            let bytes = val.to_be_bytes();
            let (hi, lo) = bytes.split_at(8);
            ScVal::I128(Int128Parts {
                hi: i64::from_be_bytes(hi.try_into()?),
                lo: u64::from_be_bytes(lo.try_into()?),
            })
        }

        // Number parsing
        (ScType::U256, Value::String(s)) => {
            let (hi, lo) = ethnum::U256::from_str_prefixed(s)?.into_words();
            let hi_bytes = hi.to_be_bytes();
            let (hi_hi, hi_lo) = hi_bytes.split_at(8);
            let lo_bytes = lo.to_be_bytes();
            let (lo_hi, lo_lo) = lo_bytes.split_at(8);
            ScVal::U256(UInt256Parts {
                hi_hi: u64::from_be_bytes(hi_hi.try_into()?),
                hi_lo: u64::from_be_bytes(hi_lo.try_into()?),
                lo_hi: u64::from_be_bytes(lo_hi.try_into()?),
                lo_lo: u64::from_be_bytes(lo_lo.try_into()?),
            })
        }
        (ScType::I256, Value::String(s)) => {
            let (hi, lo) = ethnum::I256::from_str_prefixed(s)?.into_words();
            let hi_bytes = hi.to_be_bytes();
            let (hi_hi, hi_lo) = hi_bytes.split_at(8);
            let lo_bytes = lo.to_be_bytes();
            let (lo_hi, lo_lo) = lo_bytes.split_at(8);
            ScVal::I256(Int256Parts {
                hi_hi: i64::from_be_bytes(hi_hi.try_into()?),
                hi_lo: u64::from_be_bytes(hi_lo.try_into()?),
                lo_hi: u64::from_be_bytes(lo_hi.try_into()?),
                lo_lo: u64::from_be_bytes(lo_lo.try_into()?),
            })
        }

        (ScType::I32, Value::Number(n)) => ScVal::I32(
            n.as_i64()
                .ok_or_else(|| Error::InvalidValue(Some(t.clone())))?
                .try_into()
                .map_err(|_| Error::InvalidValue(Some(t.clone())))?,
        ),
        (ScType::U32, Value::Number(n)) => ScVal::U32(
            n.as_u64()
                .ok_or_else(|| Error::InvalidValue(Some(t.clone())))?
                .try_into()
                .map_err(|_| Error::InvalidValue(Some(t.clone())))?,
        ),
        (ScType::I64, Value::Number(n)) => ScVal::I64(
            n.as_i64()
                .ok_or_else(|| Error::InvalidValue(Some(t.clone())))?,
        ),
        (ScType::U64 | ScType::Timepoint | ScType::Duration, Value::Number(n)) => ScVal::U64(
            n.as_u64()
                .ok_or_else(|| Error::InvalidValue(Some(t.clone())))?,
        ),

        // Symbol parsing
        (ScType::Symbol, Value::String(s)) => ScVal::Symbol(ScSymbol(
            s.as_bytes()
                .try_into()
                .map_err(|_| Error::InvalidValue(Some(t.clone())))?,
        )),

        (ScType::Address | ScType::MuxedAddress, Value::String(s)) => sc_address_from_json(s)?,

        // Bytes parsing
        (bytes @ ScType::BytesN(_), Value::Number(n)) => {
            from_json_primitives(&Value::String(format!("{n}")), bytes)?
        }
        (ScType::BytesN(bytes), Value::String(s)) => ScVal::Bytes(ScBytes({
            if bytes.n == 32 {
                // Bytes might be a strkey, try parsing it as one. Contract devs should use the new
                // proper Address type, but for backwards compatibility some contracts might use a
                // BytesN<32> to represent an Address.
                if let Ok(key) = sc_address_from_json(s) {
                    return Ok(key);
                }
            }
            // Bytes are not an address, just parse as a hex string
            utils::padded_hex_from_str(s, bytes.n as usize)
                .map_err(|_| Error::InvalidValue(Some(t.clone())))?
                .try_into()
                .map_err(|_| Error::InvalidValue(Some(t.clone())))?
        })),
        (ScType::Bytes, Value::Number(n)) => {
            from_json_primitives(&Value::String(format!("{n}")), &ScType::Bytes)?
        }
        (ScType::Bytes, Value::String(s)) => ScVal::Bytes(
            hex::decode(s)
                .map_err(|_| Error::InvalidValue(Some(t.clone())))?
                .try_into()
                .map_err(|_| Error::InvalidValue(Some(t.clone())))?,
        ),
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
            let converted: BytesM<{ u32::MAX }> = b?.try_into().map_err(Error::Xdr)?;
            ScVal::Bytes(ScBytes(converted))
        }

        (ScType::String, Value::String(s)) => ScVal::String(ScString(
            s.try_into()
                .map_err(|_| Error::InvalidValue(Some(t.clone())))?,
        )),
        // Todo make proper error Which shouldn't exist
        (_, raw) => serde_json::from_value(raw.clone())?,
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
        ScVal::LedgerKeyContractInstance => "LedgerKeyContractInstance".to_string(),
        _ => serde_json::to_string(&to_json(v)?)?,
    })
}

/// # Errors
///
/// Might return an error
#[allow(clippy::too_many_lines)]
pub fn to_json(v: &ScVal) -> Result<Value, Error> {
    #[allow(clippy::match_same_arms)]
    let val: Value = match v {
        ScVal::Bool(b) => Value::Bool(*b),
        ScVal::Void => Value::Null,
        ScVal::LedgerKeyContractInstance => Value::String("LedgerKeyContractInstance".to_string()),
        ScVal::U64(v) => Value::Number(serde_json::Number::from(*v)),
        ScVal::Timepoint(tp) => Value::Number(serde_json::Number::from(tp.0)),
        ScVal::Duration(d) => Value::Number(serde_json::Number::from(d.0)),
        ScVal::I64(v) => Value::Number(serde_json::Number::from(*v)),
        ScVal::U32(v) => Value::Number(serde_json::Number::from(*v)),
        ScVal::I32(v) => Value::Number(serde_json::Number::from(*v)),
        ScVal::Symbol(v) => Value::String(
            std::str::from_utf8(v.as_slice())
                .map_err(|_| Error::InvalidValue(Some(ScType::Symbol)))?
                .to_string(),
        ),
        ScVal::String(v) => Value::String(
            std::str::from_utf8(v.as_slice())
                .map_err(|_| Error::InvalidValue(Some(ScType::Symbol)))?
                .to_string(),
        ),
        ScVal::Vec(v) => {
            let values: Result<Vec<Value>, Error> = v.as_ref().map_or_else(
                || Ok(vec![]),
                |v| {
                    v.iter()
                        .map(|item| -> Result<Value, Error> { to_json(item) })
                        .collect()
                },
            );
            Value::Array(values?)
        }
        ScVal::Map(None) => Value::Object(serde_json::Map::with_capacity(0)),
        ScVal::Map(Some(v)) => {
            // TODO: What do we do if the key is not a string?
            let mut m = serde_json::Map::<String, Value>::with_capacity(v.len());
            for ScMapEntry { key, val } in v.iter() {
                let k: String = to_string(key)?;
                let v: Value = to_json(val).map_err(|_| Error::InvalidValue(None))?;
                m.insert(k, v);
            }
            Value::Object(m)
        }
        ScVal::Bytes(v) => Value::String(to_lower_hex(v.as_slice())),
        ScVal::Address(v) => sc_address_to_json(v),
        ScVal::U128(n) => {
            let hi: [u8; 8] = n.hi.to_be_bytes();
            let lo: [u8; 8] = n.lo.to_be_bytes();
            let bytes = [hi, lo].concat();
            // Always output u128s as strings
            let v = u128::from_be_bytes(
                bytes
                    .as_slice()
                    .try_into()
                    .map_err(|_| Error::InvalidValue(Some(ScType::I128)))?,
            )
            .to_string();
            Value::String(v)
        }
        ScVal::I128(n) => {
            let hi: [u8; 8] = n.hi.to_be_bytes();
            let lo: [u8; 8] = n.lo.to_be_bytes();
            let bytes = [hi, lo].concat();
            // Always output u128s as strings
            let v = i128::from_be_bytes(
                bytes
                    .as_slice()
                    .try_into()
                    .map_err(|_| Error::InvalidValue(Some(ScType::I128)))?,
            )
            .to_string();
            Value::String(v)
        }
        ScVal::U256(u256parts) => {
            let bytes = [
                u256parts.hi_hi.to_be_bytes(),
                u256parts.hi_lo.to_be_bytes(),
                u256parts.lo_hi.to_be_bytes(),
                u256parts.lo_lo.to_be_bytes(),
            ]
            .concat();
            let u256 = ethnum::U256::from_be_bytes(
                bytes
                    .as_slice()
                    .try_into()
                    .map_err(|_| Error::InvalidValue(Some(ScType::U256)))?,
            );
            Value::String(u256.to_string())
        }
        ScVal::I256(i256parts) => {
            let bytes = [
                i256parts.hi_hi.to_be_bytes(),
                i256parts.hi_lo.to_be_bytes(),
                i256parts.lo_hi.to_be_bytes(),
                i256parts.lo_lo.to_be_bytes(),
            ]
            .concat();
            let i256 = ethnum::I256::from_be_bytes(
                bytes
                    .as_slice()
                    .try_into()
                    .map_err(|_| Error::InvalidValue(Some(ScType::I256)))?,
            );
            Value::String(i256.to_string())
        }
        ScVal::ContractInstance(ScContractInstance {
            executable: ContractExecutable::Wasm(hash),
            ..
        }) => json!({ "hash": hash }),
        ScVal::ContractInstance(ScContractInstance {
            executable: ContractExecutable::StellarAsset,
            ..
        }) => json!({"SAC": true}),
        ScVal::LedgerKeyNonce(ScNonceKey { nonce }) => {
            Value::Number(serde_json::Number::from(*nonce))
        }
        ScVal::Error(e) => serde_json::to_value(e)?,
    };
    Ok(val)
}

fn sc_address_to_json(v: &ScAddress) -> Value {
    match v {
        ScAddress::Account(AccountId(PublicKey::PublicKeyTypeEd25519(Uint256(k)))) => {
            Value::String(stellar_strkey::ed25519::PublicKey(*k).to_string())
        }
        ScAddress::Contract(ContractId(h)) => {
            Value::String(stellar_strkey::Contract(h.clone().into()).to_string())
        }
        ScAddress::MuxedAccount(MuxedEd25519Account { ed25519, id }) => Value::String(
            stellar_strkey::ed25519::MuxedAccount {
                ed25519: ed25519.0,
                id: *id,
            }
            .to_string(),
        ),
        ScAddress::ClaimableBalance(_) => todo!("ClaimableBalance is not supported"),
        ScAddress::LiquidityPool(_) => todo!("LiquidityPool is not supported"),
    }
}

fn sc_address_from_json(s: &str) -> Result<ScVal, Error> {
    stellar_strkey::Strkey::from_string(s)
        .map_err(|_| Error::InvalidValue(Some(ScType::Address)))
        .map(|parsed| match parsed {
            stellar_strkey::Strkey::PublicKeyEd25519(p) => Some(ScVal::Address(
                ScAddress::Account(AccountId(PublicKey::PublicKeyTypeEd25519(Uint256(p.0)))),
            )),
            stellar_strkey::Strkey::Contract(c) => {
                Some(ScVal::Address(ScAddress::Contract(ContractId(Hash(c.0)))))
            }
            stellar_strkey::Strkey::MuxedAccountEd25519(m) => Some(ScVal::Address(
                ScAddress::MuxedAccount(MuxedEd25519Account {
                    ed25519: Uint256(m.ed25519),
                    id: m.id,
                }),
            )),
            _ => None,
        })?
        .ok_or(Error::InvalidValue(Some(ScType::Address)))
}

fn to_lower_hex(bytes: &[u8]) -> String {
    let mut res = String::with_capacity(bytes.len());
    for b in bytes {
        let _ = write!(res, "{b:02x}");
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
            ScType::Error => Some("Error".to_string()),
            ScType::Bytes => Some("hex_bytes".to_string()),
            ScType::Address => Some("Address".to_string()),
            ScType::Void => Some("Null".to_string()),
            ScType::Timepoint => Some("Timepoint".to_string()),
            ScType::Duration => Some("Duration".to_string()),
            ScType::U256 => Some("u256".to_string()),
            ScType::I256 => Some("i256".to_string()),
            ScType::String => Some("String".to_string()),
            ScType::MuxedAddress => Some("MuxedAddress".to_string()),
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
                match self.find(&name.to_utf8_string_lossy()).ok()? {
                    ScSpecEntry::UdtStructV0(ScSpecUdtStructV0 { fields, .. })
                        if fields
                            .first()
                            .is_some_and(|f| f.name.to_utf8_string_lossy() == "0") =>
                    {
                        let fields = fields
                            .iter()
                            .map(|t| self.arg_value_name(&t.type_, depth + 1))
                            .collect::<Option<Vec<_>>>()?
                            .join(", ");
                        Some(format!("[{fields}]"))
                    }
                    ScSpecEntry::UdtStructV0(strukt) => self.arg_value_udt(strukt, depth),
                    ScSpecEntry::UdtUnionV0(union) => self.arg_value_union(union, depth),
                    ScSpecEntry::UdtEnumV0(enum_) => Some(arg_value_enum(enum_)),
                    ScSpecEntry::FunctionV0(_)
                    | ScSpecEntry::UdtErrorEnumV0(_)
                    | ScSpecEntry::EventV0(_) => None,
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
            .map(|f| (f.name.to_utf8_string_lossy(), &f.type_))
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
                    ScSpecUdtUnionCaseV0::VoidV0(ScSpecUdtUnionCaseVoidV0 { name, .. }) => {
                        name.to_utf8_string_lossy()
                    }
                    ScSpecUdtUnionCaseV0::TupleV0(ScSpecUdtUnionCaseTupleV0 {
                        name,
                        type_,
                        ..
                    }) => {
                        if depth > 1 {
                            match &type_[0] {
                                ScType::Vec(type_vec) => {
                                    let element_type = type_vec.element_type.clone();
                                    if let ScType::Udt(udt_type) = *element_type {
                                        return Some(udt_type.name.to_utf8_string_lossy());
                                    }
                                }
                                _ => {
                                    return Some(format!(
                                        "{}(ScSpecTypeDef::{})",
                                        name.to_utf8_string_lossy(),
                                        type_[0].name()
                                    ))
                                }
                            }
                            format!("{}({})", name.to_utf8_string_lossy(), type_[0].name())
                        } else {
                            format!(
                                "{}({})",
                                name.to_utf8_string_lossy(),
                                type_
                                    .iter()
                                    .map(|type_| self.arg_value_name(type_, depth + 1))
                                    .collect::<Option<Vec<String>>>()?
                                    .join(",")
                            )
                        }
                    }
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
    pub fn example(&self, depth: usize, type_: &ScType) -> Option<String> {
        #[allow(clippy::match_same_arms)]
        match type_ {
            ScType::U64 => Some("1".to_string()),
            ScType::I64 => Some("1".to_string()),
            ScType::U128 => Some("\"1\"".to_string()),
            ScType::I128 => Some("\"1\"".to_string()),
            ScType::U32 => Some("1".to_string()),
            ScType::I32 => Some("1".to_string()),
            ScType::Bool => Some("true".to_string()),
            ScType::Symbol => Some("\"hello\"".to_string()),
            ScType::Error => Some("Error".to_string()),
            ScType::Bytes => Some("\"0000000000000000\"".to_string()),
            ScType::Address => {
                Some("\"GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF\"".to_string())
            }
            ScType::Void => Some("null".to_string()),
            ScType::Timepoint => Some("1743010492".to_string()),
            ScType::Duration => Some("1".to_string()),
            ScType::U256 => Some("\"1\"".to_string()),
            ScType::I256 => Some("\"1\"".to_string()),
            ScType::String => Some("\"hello world\"".to_string()),
            ScType::Option(val) => {
                let ScSpecTypeOption { value_type } = val.as_ref();
                self.example(depth, value_type.as_ref())
            }
            ScType::Vec(val) => {
                let ScSpecTypeVec { element_type } = val.as_ref();
                let inner = self.example(depth, element_type.as_ref())?;
                Some(format!("[ {inner} ]"))
            }
            ScType::Result(val) => {
                let ScSpecTypeResult {
                    ok_type,
                    error_type,
                } = val.as_ref();
                let ok = self.example(depth, ok_type.as_ref())?;
                let error = self.example(depth, error_type.as_ref())?;
                Some(format!("Result<{ok}, {error}>"))
            }
            ScType::Tuple(val) => {
                let ScSpecTypeTuple { value_types } = val.as_ref();
                let names = value_types
                    .iter()
                    .map(|t| self.example(depth, t))
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
                    self.example(depth, key_type.as_ref())?,
                    self.example(depth, value_type.as_ref())?,
                );
                if !matches!(key_type.as_ref(), ScType::Symbol) {
                    key = format!("\"{key}\"");
                }
                Some(format!("{{ {key}: {val} }}"))
            }
            ScType::BytesN(n) => {
                let n = n.n as usize;
                let res = "00".repeat(n);
                Some(format!("\"{res}\""))
            }
            ScType::Udt(ScSpecTypeUdt { name }) => {
                self.example_udts(depth, name.to_utf8_string_lossy().as_ref())
            }
            ScType::MuxedAddress => {
                Some("\"GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF\"".to_string())
            }
            // No specific value name for these yet.
            ScType::Val => None,
        }
    }

    fn example_udts(&self, depth: usize, name: &str) -> Option<String> {
        if depth > 2 {
            return Some(name.to_string());
        }

        let depth = depth + 1;
        let built = match self.find(name).ok() {
            Some(ScSpecEntry::UdtStructV0(strukt)) => {
                // Check if a tuple strukt and handle it just as a tuple going forward
                let build_struct = if !strukt.fields.is_empty()
                    && strukt.fields[0].name.to_utf8_string_lossy() == "0"
                {
                    let value_types = strukt
                        .fields
                        .iter()
                        .map(|f| f.type_.clone())
                        .collect::<Vec<_>>()
                        .try_into()
                        .ok()?;
                    self.example(
                        depth,
                        &ScType::Tuple(Box::new(ScSpecTypeTuple { value_types })),
                    )
                } else {
                    let inner = strukt
                        .fields
                        .iter()
                        .map(|f| (f.name.to_utf8_string_lossy(), &f.type_))
                        .map(|(name, type_)| {
                            let type_ = self.example(depth, type_)?;
                            let name = format!(r#""{name}""#);
                            Some(format!("{name}: {type_}"))
                        })
                        .collect::<Option<Vec<_>>>()?
                        .join(", ");
                    Some(format!(r"{{ {inner} }}"))
                };
                build_struct
            }
            Some(ScSpecEntry::UdtUnionV0(union)) => self.example_union(depth, union),
            Some(ScSpecEntry::UdtEnumV0(enum_)) => {
                enum_.cases.iter().next().map(|c| c.value.to_string())
            }
            Some(
                ScSpecEntry::FunctionV0(_)
                | ScSpecEntry::UdtErrorEnumV0(_)
                | ScSpecEntry::EventV0(_),
            )
            | None => None,
        };

        built
    }

    fn example_union(&self, depth: usize, union: &ScSpecUdtUnionV0) -> Option<String> {
        let res = union
            .cases
            .iter()
            .map(|case| match case {
                ScSpecUdtUnionCaseV0::VoidV0(ScSpecUdtUnionCaseVoidV0 { name, .. }) => {
                    Some(format!("\"{}\"", name.to_utf8_string_lossy()))
                }
                ScSpecUdtUnionCaseV0::TupleV0(ScSpecUdtUnionCaseTupleV0 {
                    name, type_, ..
                }) => {
                    if type_.len() == 1 {
                        let single = self.example(depth, &type_[0])?;
                        Some(format!("{{\"{}\":{single}}}", name.to_utf8_string_lossy()))
                    } else {
                        let names = type_
                            .iter()
                            .map(|t| self.example(depth, t))
                            .collect::<Option<Vec<_>>>()?
                            .join(", ");
                        Some(format!("{{\"{}\":[{names}]}}", name.to_utf8_string_lossy()))
                    }
                }
            })
            .collect::<Option<Vec<_>>>()?;
        Some(res.join("|"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use stellar_xdr::curr::ScSpecTypeBytesN;

    #[test]
    fn from_json_primitives_bytesn() {
        // TODO: Add test for parsing addresses

        // Check it parses hex-encoded bytes
        let b = from_json_primitives(
            &Value::String("beefface".to_string()),
            &ScType::BytesN(ScSpecTypeBytesN { n: 4 }),
        )
        .unwrap();
        assert_eq!(
            b,
            ScVal::Bytes(ScBytes(vec![0xbe, 0xef, 0xfa, 0xce].try_into().unwrap()))
        );

        // Check it parses hex-encoded bytes when they are all numbers. Normally the json would
        // interpret the CLI arg as a number, so we need a special case there.
        let b = from_json_primitives(
            &Value::Number(4554.into()),
            &ScType::BytesN(ScSpecTypeBytesN { n: 2 }),
        )
        .unwrap();
        assert_eq!(
            b,
            ScVal::Bytes(ScBytes(vec![0x45, 0x54].try_into().unwrap()))
        );
    }

    #[test]
    fn from_json_primitives_bytes() {
        // Check it parses hex-encoded bytes
        let b =
            from_json_primitives(&Value::String("beefface".to_string()), &ScType::Bytes).unwrap();
        assert_eq!(
            b,
            ScVal::Bytes(ScBytes(vec![0xbe, 0xef, 0xfa, 0xce].try_into().unwrap()))
        );

        // Check it parses hex-encoded bytes when they are all numbers. Normally the json would
        // interpret the CLI arg as a number, so we need a special case there.
        let b = from_json_primitives(&Value::Number(4554.into()), &ScType::Bytes).unwrap();
        assert_eq!(
            b,
            ScVal::Bytes(ScBytes(vec![0x45, 0x54].try_into().unwrap()))
        );
    }

    #[test]
    fn test_sc_address_from_json_strkey() {
        // All zero contract address
        match sc_address_from_json("CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4") {
            Ok(addr) => assert_eq!(
                addr,
                ScVal::Address(ScAddress::Contract(ContractId(Hash([0; 32]))))
            ),
            Err(e) => panic!("Unexpected error: {e}"),
        }

        // Real contract address
        match sc_address_from_json("CA3D5KRYM6CB7OWQ6TWYRR3Z4T7GNZLKERYNZGGA5SOAOPIFY6YQGAXE") {
            Ok(addr) => assert_eq!(
                addr,
                ScVal::Address(ScAddress::Contract(ContractId::from(Hash([
                    0x36, 0x3e, 0xaa, 0x38, 0x67, 0x84, 0x1f, 0xba, 0xd0, 0xf4, 0xed, 0x88, 0xc7,
                    0x79, 0xe4, 0xfe, 0x66, 0xe5, 0x6a, 0x24, 0x70, 0xdc, 0x98, 0xc0, 0xec, 0x9c,
                    0x07, 0x3d, 0x05, 0xc7, 0xb1, 0x03,
                ]))))
            ),
            Err(e) => panic!("Unexpected error: {e}"),
        }

        // All zero user account address
        match sc_address_from_json("GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF") {
            Ok(addr) => assert_eq!(
                addr,
                ScVal::Address(ScAddress::Account(AccountId(
                    PublicKey::PublicKeyTypeEd25519([0; 32].into())
                )))
            ),
            Err(e) => panic!("Unexpected error: {e}"),
        }

        // Real user account address
        match sc_address_from_json("GA3D5KRYM6CB7OWQ6TWYRR3Z4T7GNZLKERYNZGGA5SOAOPIFY6YQHES5") {
            Ok(addr) => assert_eq!(
                addr,
                ScVal::Address(ScAddress::Account(AccountId(
                    PublicKey::PublicKeyTypeEd25519(
                        [
                            0x36, 0x3e, 0xaa, 0x38, 0x67, 0x84, 0x1f, 0xba, 0xd0, 0xf4, 0xed, 0x88,
                            0xc7, 0x79, 0xe4, 0xfe, 0x66, 0xe5, 0x6a, 0x24, 0x70, 0xdc, 0x98, 0xc0,
                            0xec, 0x9c, 0x07, 0x3d, 0x05, 0xc7, 0xb1, 0x03,
                        ]
                        .into()
                    )
                )))
            ),
            Err(e) => panic!("Unexpected error: {e}"),
        }

        // MuxedAddress test
        match sc_address_from_json(
            "MA5XIGA5C7QTPTWXQHY6MCJRMTRZDOSHR6EFIBNDQTCQHG262N4GGGC2XZXD7G7EWH7U6",
        ) {
            Ok(addr) => {
                // This should parse successfully now
                assert!(matches!(addr, ScVal::Address(ScAddress::MuxedAccount(_))));
            }
            Err(e) => panic!("Unexpected error parsing MuxedAddress: {e}"),
        }
    }

    #[test]
    fn test_sc_address_json_strkey_muxed_roundtrip() {
        let muxed_address_str =
            "MA5XIGA5C7QTPTWXQHY6MCJRMTRZDOSHR6EFIBNDQTCQHG262N4GGGC2XZXD7G7EWH7U6";

        // First convert from JSON strkey to ScAddress, then convert back to JSON strkey using sc_address_to_json
        match sc_address_from_json(muxed_address_str) {
            Ok(ScVal::Address(address)) => {
                let json_result = sc_address_to_json(&address);
                assert_eq!(json_result, Value::String(muxed_address_str.to_string()));
            }
            Ok(_) => panic!("Expected ScVal::Address"),
            Err(e) => panic!("Unexpected error: {e}"),
        }
    }

    #[test]
    fn test_sc_muxed_address_from_json() {
        let expected_ed25519 = Uint256([
            0x3b, 0x74, 0x18, 0x1d, 0x17, 0xe1, 0x37, 0xce, 0xd7, 0x81, 0xf1, 0xe6, 0x09, 0x31,
            0x64, 0xe3, 0x91, 0xba, 0x47, 0x8f, 0x88, 0x54, 0x05, 0xa3, 0x84, 0xc5, 0x03, 0x9b,
            0x5e, 0xd3, 0x78, 0x63,
        ]);
        let expected_id = 1_754_924_385_537_090_737_u64;

        // Generate the muxed address from the expected values
        let muxed_addr = stellar_strkey::ed25519::MuxedAccount {
            ed25519: expected_ed25519.0,
            id: expected_id,
        }
        .to_string();

        // Test muxed address parsing
        match sc_address_from_json(&muxed_addr) {
            Ok(ScVal::Address(ScAddress::MuxedAccount(MuxedEd25519Account { ed25519, id }))) => {
                // Verify the actual content of the MuxedEd25519Account
                assert_eq!(ed25519, expected_ed25519);
                assert_eq!(id, expected_id);
            }
            Ok(_) => panic!("Expected ScVal::Address(ScAddress::MuxedAccount(_))"),
            Err(e) => panic!("Unexpected error parsing MuxedAddress: {e}"),
        }
    }

    #[test]
    fn test_u128_conversions() {
        // Test basic positive value
        let val = 42u128;
        let json_val = Value::String(val.to_string());
        let scval = from_json_primitives(&json_val, &ScType::U128).unwrap();
        assert_eq!(to_json(&scval).unwrap(), json_val);

        // Test zero
        let val = 0u128;
        let json_val = Value::String(val.to_string());
        let scval = from_json_primitives(&json_val, &ScType::U128).unwrap();
        assert_eq!(to_json(&scval).unwrap(), json_val);

        // Test maximum value
        let val = u128::MAX;
        let json_val = Value::String(val.to_string());
        let scval = from_json_primitives(&json_val, &ScType::U128).unwrap();
        assert_eq!(to_json(&scval).unwrap(), json_val);

        // Test large value
        let val = 340_282_366_920_938_463_463_374_607_431_768_211_455u128;
        let json_val = Value::String(val.to_string());
        let scval = from_json_primitives(&json_val, &ScType::U128).unwrap();
        assert_eq!(to_json(&scval).unwrap(), json_val);
    }

    #[test]
    fn test_i128_conversions() {
        // Test basic positive value
        let val = 42i128;
        let json_val = Value::String(val.to_string());
        let scval = from_json_primitives(&json_val, &ScType::I128).unwrap();
        assert_eq!(to_json(&scval).unwrap(), json_val);

        // Test basic negative value
        let val = -42i128;
        let json_val = Value::String(val.to_string());
        let scval = from_json_primitives(&json_val, &ScType::I128).unwrap();
        assert_eq!(to_json(&scval).unwrap(), json_val);

        // Test zero
        let val = 0i128;
        let json_val = Value::String(val.to_string());
        let scval = from_json_primitives(&json_val, &ScType::I128).unwrap();
        assert_eq!(to_json(&scval).unwrap(), json_val);

        // Test maximum value
        let val = i128::MAX;
        let json_val = Value::String(val.to_string());
        let scval = from_json_primitives(&json_val, &ScType::I128).unwrap();
        assert_eq!(to_json(&scval).unwrap(), json_val);

        // Test minimum value
        let val = i128::MIN;
        let json_val = Value::String(val.to_string());
        let scval = from_json_primitives(&json_val, &ScType::I128).unwrap();
        assert_eq!(to_json(&scval).unwrap(), json_val);

        // Test large negative value
        let val = -170_141_183_460_469_231_731_687_303_715_884_105_728i128;
        let json_val = Value::String(val.to_string());
        let scval = from_json_primitives(&json_val, &ScType::I128).unwrap();
        assert_eq!(to_json(&scval).unwrap(), json_val);
    }

    #[test]
    fn test_u256_conversions() {
        // Test basic positive value
        let val = "42";
        let json_val = Value::String(val.to_string());
        let scval = from_json_primitives(&json_val, &ScType::U256).unwrap();
        assert_eq!(to_json(&scval).unwrap(), json_val);

        // Test zero
        let val = "0";
        let json_val = Value::String(val.to_string());
        let scval = from_json_primitives(&json_val, &ScType::U256).unwrap();
        assert_eq!(to_json(&scval).unwrap(), json_val);

        // Test large value
        let val = "115792089237316195423570985008687907853269984665640564039457584007913129639935";
        let json_val = Value::String(val.to_string());
        let scval = from_json_primitives(&json_val, &ScType::U256).unwrap();
        assert_eq!(to_json(&scval).unwrap(), json_val);

        // Test hex format
        let val = "0xff";
        let json_val = Value::String(val.to_string());
        let scval = from_json_primitives(&json_val, &ScType::U256).unwrap();
        let expected = Value::String("255".to_string());
        assert_eq!(to_json(&scval).unwrap(), expected);

        // Test hex format with 0x prefix
        let val = "0x1234567890abcdef";
        let json_val = Value::String(val.to_string());
        let scval = from_json_primitives(&json_val, &ScType::U256).unwrap();
        let expected = Value::String("1311768467294899695".to_string());
        assert_eq!(to_json(&scval).unwrap(), expected);
    }

    #[test]
    fn test_i256_conversions() {
        // Test basic positive value
        let val = "42";
        let json_val = Value::String(val.to_string());
        let scval = from_json_primitives(&json_val, &ScType::I256).unwrap();
        assert_eq!(to_json(&scval).unwrap(), json_val);

        // Test basic negative value
        let val = "-42";
        let json_val = Value::String(val.to_string());
        let scval = from_json_primitives(&json_val, &ScType::I256).unwrap();
        assert_eq!(to_json(&scval).unwrap(), json_val);

        // Test zero
        let val = "0";
        let json_val = Value::String(val.to_string());
        let scval = from_json_primitives(&json_val, &ScType::I256).unwrap();
        assert_eq!(to_json(&scval).unwrap(), json_val);

        // Test large positive value
        let val = "57896044618658097711785492504343953926634992332820282019728792003956564819967";
        let json_val = Value::String(val.to_string());
        let scval = from_json_primitives(&json_val, &ScType::I256).unwrap();
        assert_eq!(to_json(&scval).unwrap(), json_val);

        // Test large negative value
        let val = "-57896044618658097711785492504343953926634992332820282019728792003956564819968";
        let json_val = Value::String(val.to_string());
        let scval = from_json_primitives(&json_val, &ScType::I256).unwrap();
        assert_eq!(to_json(&scval).unwrap(), json_val);

        // Test hex format
        let val = "0xff";
        let json_val = Value::String(val.to_string());
        let scval = from_json_primitives(&json_val, &ScType::I256).unwrap();
        let expected = Value::String("255".to_string());
        assert_eq!(to_json(&scval).unwrap(), expected);

        // Test negative hex format
        let val = "-0xff";
        let json_val = Value::String(val.to_string());
        let scval = from_json_primitives(&json_val, &ScType::I256).unwrap();
        let expected = Value::String("-255".to_string());
        assert_eq!(to_json(&scval).unwrap(), expected);
    }

    #[test]
    fn test_integer_conversion_edge_cases() {
        // Test invalid string for U128
        let json_val = Value::String("not_a_number".to_string());
        let result = from_json_primitives(&json_val, &ScType::U128);
        assert!(result.is_err());

        // Test invalid string for I128
        let json_val = Value::String("not_a_number".to_string());
        let result = from_json_primitives(&json_val, &ScType::I128);
        assert!(result.is_err());

        // Test invalid string for U256
        let json_val = Value::String("not_a_number".to_string());
        let result = from_json_primitives(&json_val, &ScType::U256);
        assert!(result.is_err());

        // Test invalid string for I256
        let json_val = Value::String("not_a_number".to_string());
        let result = from_json_primitives(&json_val, &ScType::I256);
        assert!(result.is_err());

        // Test negative value for U128 (should fail)
        let json_val = Value::String("-42".to_string());
        let result = from_json_primitives(&json_val, &ScType::U128);
        assert!(result.is_err());

        // Test negative value for U256 (should fail)
        let json_val = Value::String("-42".to_string());
        let result = from_json_primitives(&json_val, &ScType::U256);
        assert!(result.is_err());
    }

    #[test]
    fn test_basic_numeric_conversions() {
        // Test U32
        let val = 42u32;
        let json_val = Value::Number(val.into());
        let scval = from_json_primitives(&json_val, &ScType::U32).unwrap();
        assert_eq!(to_json(&scval).unwrap(), json_val);

        // Test I32
        let val = -42i32;
        let json_val = Value::Number(val.into());
        let scval = from_json_primitives(&json_val, &ScType::I32).unwrap();
        assert_eq!(to_json(&scval).unwrap(), json_val);

        // Test U64
        let val = 42u64;
        let json_val = Value::Number(val.into());
        let scval = from_json_primitives(&json_val, &ScType::U64).unwrap();
        assert_eq!(to_json(&scval).unwrap(), json_val);

        // Test I64
        let val = -42i64;
        let json_val = Value::Number(val.into());
        let scval = from_json_primitives(&json_val, &ScType::I64).unwrap();
        assert_eq!(to_json(&scval).unwrap(), json_val);

        // Test boolean
        let scval = from_json_primitives(&Value::Bool(true), &ScType::Bool).unwrap();
        assert_eq!(to_json(&scval).unwrap(), Value::Bool(true));

        let scval = from_json_primitives(&Value::Bool(false), &ScType::Bool).unwrap();
        assert_eq!(to_json(&scval).unwrap(), Value::Bool(false));
    }

    #[test]
    fn test_numeric_extremes() {
        // Test U32 maximum
        let val = u32::MAX;
        let json_val = Value::Number(val.into());
        let scval = from_json_primitives(&json_val, &ScType::U32).unwrap();
        assert_eq!(to_json(&scval).unwrap(), json_val);

        // Test I32 minimum and maximum
        let val = i32::MIN;
        let json_val = Value::Number(val.into());
        let scval = from_json_primitives(&json_val, &ScType::I32).unwrap();
        assert_eq!(to_json(&scval).unwrap(), json_val);

        let val = i32::MAX;
        let json_val = Value::Number(val.into());
        let scval = from_json_primitives(&json_val, &ScType::I32).unwrap();
        assert_eq!(to_json(&scval).unwrap(), json_val);

        // Test U64 maximum
        let val = u64::MAX;
        let json_val = Value::Number(val.into());
        let scval = from_json_primitives(&json_val, &ScType::U64).unwrap();
        assert_eq!(to_json(&scval).unwrap(), json_val);

        // Test I64 minimum and maximum
        let val = i64::MIN;
        let json_val = Value::Number(val.into());
        let scval = from_json_primitives(&json_val, &ScType::I64).unwrap();
        assert_eq!(to_json(&scval).unwrap(), json_val);

        let val = i64::MAX;
        let json_val = Value::Number(val.into());
        let scval = from_json_primitives(&json_val, &ScType::I64).unwrap();
        assert_eq!(to_json(&scval).unwrap(), json_val);
    }

    #[test]
    fn test_string_primitive_integration() {
        // Test that from_string_primitive works with various types
        // U128
        let val = "42";
        let scval = from_string_primitive(val, &ScType::U128).unwrap();
        assert_eq!(to_string(&scval).unwrap(), "\"42\"");

        // I128
        let val = "-42";
        let scval = from_string_primitive(val, &ScType::I128).unwrap();
        assert_eq!(to_string(&scval).unwrap(), "\"-42\"");

        // U256
        let val = "12345678901234567890123456789012345678901234567890";
        let scval = from_string_primitive(val, &ScType::U256).unwrap();
        assert_eq!(
            to_string(&scval).unwrap(),
            "\"12345678901234567890123456789012345678901234567890\""
        );

        // I256
        let val = "-12345678901234567890123456789012345678901234567890";
        let scval = from_string_primitive(val, &ScType::I256).unwrap();
        assert_eq!(
            to_string(&scval).unwrap(),
            "\"-12345678901234567890123456789012345678901234567890\""
        );

        // Boolean
        let scval = from_string_primitive("true", &ScType::Bool).unwrap();
        assert_eq!(to_string(&scval).unwrap(), "true");

        let scval = from_string_primitive("false", &ScType::Bool).unwrap();
        assert_eq!(to_string(&scval).unwrap(), "false");
    }
}
