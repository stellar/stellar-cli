use serde::Serialize;
use stellar_xdr::curr::{
    ScSpecEntry, ScSpecFunctionInputV0, ScSpecTypeDef, ScSpecUdtEnumCaseV0,
    ScSpecUdtErrorEnumCaseV0, ScSpecUdtStructFieldV0, ScSpecUdtStructV0, ScSpecUdtUnionCaseV0,
};

#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StructField {
    pub doc: String,
    pub name: String,
    pub value: Type,
}

impl From<&ScSpecUdtStructFieldV0> for StructField {
    fn from(f: &ScSpecUdtStructFieldV0) -> Self {
        StructField {
            doc: f.doc.to_utf8_string_lossy(),
            name: f.name.to_utf8_string_lossy(),
            value: (&f.type_).into(),
        }
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FunctionInput {
    pub doc: String,
    pub name: String,
    pub value: Type,
}

impl From<&ScSpecFunctionInputV0> for FunctionInput {
    fn from(f: &ScSpecFunctionInputV0) -> Self {
        FunctionInput {
            doc: f.doc.to_utf8_string_lossy(),
            name: f.name.to_utf8_string_lossy(),
            value: (&f.type_).into(),
        }
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnionCase {
    pub doc: String,
    pub name: String,
    pub values: Vec<Type>,
}

impl From<&ScSpecUdtUnionCaseV0> for UnionCase {
    fn from(c: &ScSpecUdtUnionCaseV0) -> Self {
        let (doc, name, values) = match c {
            ScSpecUdtUnionCaseV0::VoidV0(v) => (
                v.doc.to_utf8_string_lossy(),
                v.name.to_utf8_string_lossy(),
                vec![],
            ),
            ScSpecUdtUnionCaseV0::TupleV0(t) => (
                t.doc.to_utf8_string_lossy(),
                t.name.to_utf8_string_lossy(),
                t.type_.iter().map(Type::from).collect(),
            ),
        };
        UnionCase { doc, name, values }
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnumCase {
    pub doc: String,
    pub name: String,
    pub value: u32,
}

impl From<&ScSpecUdtEnumCaseV0> for EnumCase {
    fn from(c: &ScSpecUdtEnumCaseV0) -> Self {
        EnumCase {
            doc: c.doc.to_utf8_string_lossy(),
            name: c.name.to_utf8_string_lossy(),
            value: c.value,
        }
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorEnumCase {
    pub doc: String,
    pub name: String,
    pub value: u32,
}

impl From<&ScSpecUdtErrorEnumCaseV0> for ErrorEnumCase {
    fn from(c: &ScSpecUdtErrorEnumCaseV0) -> Self {
        ErrorEnumCase {
            doc: c.doc.to_utf8_string_lossy(),
            name: c.name.to_utf8_string_lossy(),
            value: c.value,
        }
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(tag = "type")]
#[serde(rename_all = "camelCase")]
pub enum Type {
    Void,
    Val,
    U64,
    I64,
    U32,
    I32,
    U128,
    I128,
    U256,
    I256,
    Bool,
    Symbol,
    Bytes,
    String,
    Address,
    Timepoint,
    Duration,
    Map { key: Box<Type>, value: Box<Type> },
    Option { value: Box<Type> },
    Result { value: Box<Type>, error: Box<Type> },
    Vec { element: Box<Type> },
    BytesN { n: u32 },
    Tuple { elements: Vec<Type> },
    Error { message: Option<String> },
    Custom { name: String },
    MuxedAddress,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(tag = "type")]
#[serde(rename_all = "camelCase")]
pub enum Entry {
    Function {
        doc: String,
        name: String,
        inputs: Vec<FunctionInput>,
        outputs: Vec<Type>,
    },
    Struct {
        doc: String,
        name: String,
        fields: Vec<StructField>,
    },
    TupleStruct {
        doc: String,
        name: String,
        fields: Vec<Type>,
    },
    Union {
        doc: String,
        name: String,
        cases: Vec<UnionCase>,
    },
    Enum {
        doc: String,
        name: String,
        cases: Vec<EnumCase>,
    },
    ErrorEnum {
        doc: String,
        name: String,
        cases: Vec<ErrorEnumCase>,
    },
}

impl From<&ScSpecTypeDef> for Type {
    fn from(spec: &ScSpecTypeDef) -> Self {
        match spec {
            ScSpecTypeDef::Map(map) => Type::Map {
                key: Box::new(Type::from(map.key_type.as_ref())),
                value: Box::new(Type::from(map.value_type.as_ref())),
            },
            ScSpecTypeDef::Option(opt) => Type::Option {
                value: Box::new(Type::from(opt.value_type.as_ref())),
            },
            ScSpecTypeDef::Result(res) => Type::Result {
                value: Box::new(Type::from(res.ok_type.as_ref())),
                error: Box::new(Type::from(res.error_type.as_ref())),
            },
            ScSpecTypeDef::Tuple(tuple) => Type::Tuple {
                elements: tuple.value_types.iter().map(Type::from).collect(),
            },
            ScSpecTypeDef::Vec(vec) => Type::Vec {
                element: Box::new(Type::from(vec.element_type.as_ref())),
            },
            ScSpecTypeDef::Udt(udt) => Type::Custom {
                name: udt.name.to_utf8_string_lossy(),
            },
            ScSpecTypeDef::BytesN(b) => Type::BytesN { n: b.n },
            ScSpecTypeDef::Val => Type::Val,
            ScSpecTypeDef::U64 => Type::U64,
            ScSpecTypeDef::I64 => Type::I64,
            ScSpecTypeDef::U32 => Type::U32,
            ScSpecTypeDef::I32 => Type::I32,
            ScSpecTypeDef::U128 => Type::U128,
            ScSpecTypeDef::I128 => Type::I128,
            ScSpecTypeDef::U256 => Type::U256,
            ScSpecTypeDef::I256 => Type::I256,
            ScSpecTypeDef::Bool => Type::Bool,
            ScSpecTypeDef::Symbol => Type::Symbol,
            ScSpecTypeDef::Error => Type::Error { message: None },
            ScSpecTypeDef::Bytes => Type::Bytes,
            ScSpecTypeDef::String => Type::String,
            ScSpecTypeDef::MuxedAddress => Type::MuxedAddress,
            ScSpecTypeDef::Address => Type::Address,
            ScSpecTypeDef::Void => Type::Void,
            ScSpecTypeDef::Timepoint => Type::Timepoint,
            ScSpecTypeDef::Duration => Type::Duration,
        }
    }
}

impl From<&ScSpecEntry> for Entry {
    fn from(spec: &ScSpecEntry) -> Self {
        match spec {
            ScSpecEntry::FunctionV0(f) => Entry::Function {
                doc: f.doc.to_utf8_string_lossy(),
                name: f.name.to_utf8_string_lossy(),
                inputs: f.inputs.iter().map(Into::into).collect(),
                outputs: f.outputs.iter().map(Into::into).collect(),
            },
            ScSpecEntry::UdtStructV0(s) if is_tuple_strukt(s) => Entry::TupleStruct {
                doc: s.doc.to_utf8_string_lossy(),
                name: s.name.to_utf8_string_lossy(),
                fields: s.fields.iter().map(|f| &f.type_).map(Into::into).collect(),
            },
            ScSpecEntry::UdtStructV0(s) => Entry::Struct {
                doc: s.doc.to_utf8_string_lossy(),
                name: s.name.to_utf8_string_lossy(),
                fields: s.fields.iter().map(Into::into).collect(),
            },
            ScSpecEntry::UdtUnionV0(u) => Entry::Union {
                doc: u.doc.to_utf8_string_lossy(),
                name: u.name.to_utf8_string_lossy(),
                cases: u.cases.iter().map(Into::into).collect(),
            },
            ScSpecEntry::UdtEnumV0(e) => Entry::Enum {
                doc: e.doc.to_utf8_string_lossy(),
                name: e.name.to_utf8_string_lossy(),
                cases: e.cases.iter().map(Into::into).collect(),
            },
            ScSpecEntry::UdtErrorEnumV0(e) => Entry::ErrorEnum {
                doc: e.doc.to_utf8_string_lossy(),
                name: e.name.to_utf8_string_lossy(),
                cases: e.cases.iter().map(Into::into).collect(),
            },
            ScSpecEntry::EventV0(_) => todo!("EventV0 is not implemented yet"),
        }
    }
}

fn is_tuple_strukt(s: &ScSpecUdtStructV0) -> bool {
    !s.fields.is_empty() && s.fields[0].name.to_utf8_string_lossy() == "0"
}
