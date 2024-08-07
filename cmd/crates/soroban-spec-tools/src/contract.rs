use base64::{engine::general_purpose::STANDARD as base64, Engine as _};
use itertools::Itertools;
use std::{
    fmt::Display,
    io::{self, Cursor},
};

use soroban_env_host::xdr::{
    self, Limited, Limits, ReadXdr, ScEnvMetaEntry, ScMetaEntry, ScMetaV0, ScSpecEntry,
    ScSpecFunctionV0, ScSpecUdtEnumV0, ScSpecUdtErrorEnumV0, ScSpecUdtStructV0, ScSpecUdtUnionV0,
    StringM, WriteXdr,
};
use stellar_xdr::curr::{ScSpecTypeDef, ScSpecUdtUnionCaseV0, VecM};

pub struct Spec {
    pub env_meta_base64: Option<String>,
    pub env_meta: Vec<ScEnvMetaEntry>,
    pub meta_base64: Option<String>,
    pub meta: Vec<ScMetaEntry>,
    pub spec_base64: Option<String>,
    pub spec: Vec<ScSpecEntry>,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("reading file {filepath}: {error}")]
    CannotReadContractFile {
        filepath: std::path::PathBuf,
        error: io::Error,
    },
    #[error("cannot parse wasm file {file}: {error}")]
    CannotParseWasm {
        file: std::path::PathBuf,
        error: wasmparser::BinaryReaderError,
    },
    #[error("xdr processing error: {0}")]
    Xdr(#[from] xdr::Error),

    #[error(transparent)]
    Parser(#[from] wasmparser::BinaryReaderError),
}

impl Spec {
    pub fn new(bytes: &[u8]) -> Result<Self, Error> {
        let mut env_meta: Option<&[u8]> = None;
        let mut meta: Option<&[u8]> = None;
        let mut spec: Option<&[u8]> = None;
        for payload in wasmparser::Parser::new(0).parse_all(bytes) {
            let payload = payload?;
            if let wasmparser::Payload::CustomSection(section) = payload {
                let out = match section.name() {
                    "contractenvmetav0" => &mut env_meta,
                    "contractmetav0" => &mut meta,
                    "contractspecv0" => &mut spec,
                    _ => continue,
                };
                *out = Some(section.data());
            };
        }

        let mut env_meta_base64 = None;
        let env_meta = if let Some(env_meta) = env_meta {
            env_meta_base64 = Some(base64.encode(env_meta));
            let cursor = Cursor::new(env_meta);
            let mut read = Limited::new(cursor, Limits::none());
            ScEnvMetaEntry::read_xdr_iter(&mut read).collect::<Result<Vec<_>, xdr::Error>>()?
        } else {
            vec![]
        };

        let mut meta_base64 = None;
        let meta = if let Some(meta) = meta {
            meta_base64 = Some(base64.encode(meta));
            let cursor = Cursor::new(meta);
            let mut depth_limit_read = Limited::new(cursor, Limits::none());
            ScMetaEntry::read_xdr_iter(&mut depth_limit_read)
                .collect::<Result<Vec<_>, xdr::Error>>()?
        } else {
            vec![]
        };

        let (spec_base64, spec) = if let Some(spec) = spec {
            let (spec_base64, spec) = Spec::spec_to_base64(spec)?;
            (Some(spec_base64), spec)
        } else {
            (None, vec![])
        };

        Ok(Spec {
            env_meta_base64,
            env_meta,
            meta_base64,
            meta,
            spec_base64,
            spec,
        })
    }

    pub fn spec_as_json_array(&self) -> Result<String, Error> {
        let spec = self
            .spec
            .iter()
            .map(|e| Ok(format!("\"{}\"", e.to_xdr_base64(Limits::none())?)))
            .collect::<Result<Vec<_>, Error>>()?
            .join(",\n");
        Ok(format!("[{spec}]"))
    }

    pub fn spec_to_base64(spec: &[u8]) -> Result<(String, Vec<ScSpecEntry>), Error> {
        let spec_base64 = base64.encode(spec);
        let cursor = Cursor::new(spec);
        let mut read = Limited::new(cursor, Limits::none());
        Ok((
            spec_base64,
            ScSpecEntry::read_xdr_iter(&mut read).collect::<Result<Vec<_>, xdr::Error>>()?,
        ))
    }
}

impl Display for Spec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(env_meta) = &self.env_meta_base64 {
            writeln!(f, "Env Meta: {env_meta}")?;
            for env_meta_entry in &self.env_meta {
                match env_meta_entry {
                    ScEnvMetaEntry::ScEnvMetaKindInterfaceVersion(v) => {
                        let protocol = v >> 32;
                        let interface = v & 0xffff_ffff;
                        writeln!(f, " • Interface Version: {v} (protocol: {protocol}, interface: {interface})")?;
                    }
                }
            }
            writeln!(f)?;
        } else {
            writeln!(f, "Env Meta: None\n")?;
        }

        if let Some(_meta) = &self.meta_base64 {
            writeln!(f, "Contract Meta:")?;
            for meta_entry in &self.meta {
                match meta_entry {
                    ScMetaEntry::ScMetaV0(ScMetaV0 { key, val }) => {
                        writeln!(f, " • {key}: {val}")?;
                    }
                }
            }
            writeln!(f)?;
        } else {
            writeln!(f, "Contract Meta: None\n")?;
        }

        if let Some(_spec_base64) = &self.spec_base64 {
            writeln!(f, "Contract Spec:")?;
            for spec_entry in &self.spec {
                match spec_entry {
                    ScSpecEntry::FunctionV0(func) => write_func(f, func)?,
                    ScSpecEntry::UdtUnionV0(udt) => write_union(f, udt)?,
                    ScSpecEntry::UdtStructV0(udt) => write_struct(f, udt)?,
                    ScSpecEntry::UdtEnumV0(udt) => write_enum(f, udt)?,
                    ScSpecEntry::UdtErrorEnumV0(udt) => write_error(f, udt)?,
                }
            }
        } else {
            writeln!(f, "Contract Spec: None")?;
        }
        Ok(())
    }
}

fn write_func(f: &mut std::fmt::Formatter<'_>, func: &ScSpecFunctionV0) -> std::fmt::Result {
    writeln!(f, " • Function: {}", func.name.to_utf8_string_lossy())?;
    if func.doc.len() > 0 {
        writeln!(
            f,
            "     Docs: {}",
            &indent(&func.doc.to_utf8_string_lossy(), 11).trim()
        )?;
    }
    writeln!(
        f,
        "     Inputs: {}",
        indent(&format!("{:#?}", func.inputs), 5).trim()
    )?;
    writeln!(
        f,
        "     Output: {}",
        indent(&format!("{:#?}", func.outputs), 5).trim()
    )?;
    writeln!(f)?;
    Ok(())
}

fn write_union(f: &mut std::fmt::Formatter<'_>, udt: &ScSpecUdtUnionV0) -> std::fmt::Result {
    writeln!(f, " • Union: {}", format_name(&udt.lib, &udt.name))?;
    if udt.doc.len() > 0 {
        writeln!(
            f,
            "     Docs: {}",
            indent(&udt.doc.to_utf8_string_lossy(), 10).trim()
        )?;
    }
    writeln!(f, "     Cases:")?;
    for case in udt.cases.iter() {
        writeln!(f, "      • {}", indent(&format!("{case:#?}"), 8).trim())?;
    }
    writeln!(f)?;
    Ok(())
}

fn write_struct(f: &mut std::fmt::Formatter<'_>, udt: &ScSpecUdtStructV0) -> std::fmt::Result {
    writeln!(f, " • Struct: {}", format_name(&udt.lib, &udt.name))?;
    if udt.doc.len() > 0 {
        writeln!(
            f,
            "     Docs: {}",
            indent(&udt.doc.to_utf8_string_lossy(), 10).trim()
        )?;
    }
    writeln!(f, "     Fields:")?;
    for field in udt.fields.iter() {
        writeln!(
            f,
            "      • {}: {}",
            field.name.to_utf8_string_lossy(),
            indent(&format!("{:#?}", field.type_), 8).trim()
        )?;
        if field.doc.len() > 0 {
            writeln!(f, "{}", indent(&format!("{:#?}", field.doc), 8))?;
        }
    }
    writeln!(f)?;
    Ok(())
}

fn write_enum(f: &mut std::fmt::Formatter<'_>, udt: &ScSpecUdtEnumV0) -> std::fmt::Result {
    writeln!(f, " • Enum: {}", format_name(&udt.lib, &udt.name))?;
    if udt.doc.len() > 0 {
        writeln!(
            f,
            "     Docs: {}",
            indent(&udt.doc.to_utf8_string_lossy(), 10).trim()
        )?;
    }
    writeln!(f, "     Cases:")?;
    for case in udt.cases.iter() {
        writeln!(f, "      • {}", indent(&format!("{case:#?}"), 8).trim())?;
    }
    writeln!(f)?;
    Ok(())
}

fn write_error(f: &mut std::fmt::Formatter<'_>, udt: &ScSpecUdtErrorEnumV0) -> std::fmt::Result {
    writeln!(f, " • Error: {}", format_name(&udt.lib, &udt.name))?;
    if udt.doc.len() > 0 {
        writeln!(
            f,
            "     Docs: {}",
            indent(&udt.doc.to_utf8_string_lossy(), 10).trim()
        )?;
    }
    writeln!(f, "     Cases:")?;
    for case in udt.cases.iter() {
        writeln!(f, "      • {}", indent(&format!("{case:#?}"), 8).trim())?;
    }
    writeln!(f)?;
    Ok(())
}

fn indent(s: &str, n: usize) -> String {
    let pad = " ".repeat(n);
    s.lines()
        .map(|line| format!("{pad}{line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_name(lib: &StringM<80>, name: &StringM<60>) -> String {
    if lib.len() > 0 {
        format!(
            "{}::{}",
            lib.to_utf8_string_lossy(),
            name.to_utf8_string_lossy()
        )
    } else {
        name.to_utf8_string_lossy()
    }
}

pub fn pretty_spec(spec: &Vec<ScSpecEntry>) -> String {
    let mut res = vec![
        "/////// Generated pseudocode contract spec from an XDR: \\\\\\\\\\\\\\\n".to_string(),
    ];

    let mut functions = Vec::new();

    for spec_entry in spec {
        match spec_entry {
            ScSpecEntry::FunctionV0(func) => functions.push(func),
            ScSpecEntry::UdtUnionV0(udt) => res.push(pretty_union(udt)),
            ScSpecEntry::UdtStructV0(udt) => res.push(pretty_struct(udt)),
            ScSpecEntry::UdtEnumV0(udt) => res.push(pretty_enum(udt)),
            ScSpecEntry::UdtErrorEnumV0(udt) => res.push(pretty_error(udt)),
        };
    }

    if !functions.is_empty() {
        res.push(format!(
            "pub trait ContractTrait {{\n{}}}",
            functions.iter().map(|f| pretty_func(f)).join("")
        ));
    }

    res.iter().join("\n")
}

const IDENT: &str = "    ";

fn pretty_func(func: &ScSpecFunctionV0) -> String {
    let mut res = pretty_doc(&func.doc, IDENT);

    res.push_str(&format!("{}pub fn {}(", IDENT, func.name.0));

    // TODO: handle input.doc;
    res.push_str(
        &func
            .inputs
            .as_vec()
            .iter()
            .map(|x| format!("{}: {}", x.name, pretty_type(&x.type_)))
            .join(", "),
    );

    let outputs = func.outputs.as_vec();

    res.push(')');

    match outputs.len() {
        0 => {}
        1 => res.push_str(&format!(" -> {}", pretty_type(&outputs[0]))),
        _ => unreachable!("Outputs is VecM<1>"),
    };

    res.push_str(";\n");
    res
}

fn pretty_union(udt_union: &ScSpecUdtUnionV0) -> String {
    // TODO: handle lib
    let mut res = pretty_doc(&udt_union.doc, "");

    let body = udt_union
        .cases
        .as_vec()
        .iter()
        .map(|u| match u {
            ScSpecUdtUnionCaseV0::VoidV0(void) => {
                format!("{}{}{}", pretty_doc(&void.doc, IDENT), IDENT, void.name)
            }
            ScSpecUdtUnionCaseV0::TupleV0(tuple) => format!(
                "{}{}{}{}",
                pretty_doc(&tuple.doc, IDENT),
                IDENT,
                tuple.name,
                pretty_tuple(&tuple.type_)
            ),
        })
        .join(",\n");
    res.push_str(&format!("pub enum {} {{\n{}\n}}", udt_union.name, body));

    res
}

fn pretty_struct(udt_struct: &ScSpecUdtStructV0) -> String {
    // TODO: handle lib
    let mut res = pretty_doc(&udt_struct.doc, "");

    let body = udt_struct
        .fields
        .as_vec()
        .iter()
        .map(|f| {
            format!(
                "{}{}pub {}: {}",
                pretty_doc(&f.doc, IDENT),
                IDENT,
                f.name,
                pretty_type(&f.type_)
            )
        })
        .join(",\n");

    res.push_str(&format!("pub struct {} {{\n{}\n}}", udt_struct.name, body));

    res
}

fn pretty_enum(udt: &ScSpecUdtEnumV0) -> String {
    // TODO: handle lib
    let mut res = pretty_doc(&udt.doc, "");

    let body = udt
        .cases
        .as_vec()
        .iter()
        .map(|case| {
            format!(
                "{}{}{} = {}",
                pretty_doc(&case.doc, IDENT),
                IDENT,
                case.name,
                case.value
            )
        })
        .join(",\n");

    res.push_str(&format!("pub enum {} {{\n{}\n}}", udt.name, body));

    res
}

fn pretty_error(udt: &ScSpecUdtErrorEnumV0) -> String {
    // TODO: handle lib
    let mut res = pretty_doc(&udt.doc, "");

    let body = udt
        .cases
        .as_vec()
        .iter()
        .map(|case| {
            format!(
                "{}{}{} = {}",
                pretty_doc(&case.doc, IDENT),
                IDENT,
                case.name,
                case.value
            )
        })
        .join(",\n");

    res.push_str(&format!("pub enum {} {{\n{}\n}}", udt.name, body));

    res
}

fn pretty_type(def: &ScSpecTypeDef) -> String {
    match def {
        ScSpecTypeDef::U32
        | ScSpecTypeDef::I32
        | ScSpecTypeDef::U64
        | ScSpecTypeDef::I64
        | ScSpecTypeDef::U128
        | ScSpecTypeDef::I128
        | ScSpecTypeDef::U256
        | ScSpecTypeDef::I256 => def.name().to_string().to_lowercase(),

        ScSpecTypeDef::Val
        | ScSpecTypeDef::Bool
        | ScSpecTypeDef::Void
        | ScSpecTypeDef::Error
        | ScSpecTypeDef::Timepoint
        | ScSpecTypeDef::Duration
        | ScSpecTypeDef::Bytes
        | ScSpecTypeDef::String
        | ScSpecTypeDef::Symbol
        | ScSpecTypeDef::Address => def.name().to_string(),

        ScSpecTypeDef::Option(x) => format!("Option<{}>", pretty_type(&x.value_type)),
        ScSpecTypeDef::Result(r) => format!(
            "Result<{}, {}>",
            pretty_type(&r.ok_type),
            pretty_type(&r.error_type)
        ),
        ScSpecTypeDef::Vec(v) => format!("Vec<{}>", pretty_type(&v.element_type)),
        ScSpecTypeDef::Map(m) => format!(
            "Map<{}, {}>",
            pretty_type(&m.key_type),
            pretty_type(&m.value_type)
        ),
        ScSpecTypeDef::Tuple(vec) => pretty_tuple(&vec.value_types),
        ScSpecTypeDef::BytesN(spec) => format!("BytesN<{}>", spec.n),
        ScSpecTypeDef::Udt(u) => format!("{}", u.name),
    }
}

fn pretty_doc(doc: &StringM<1024>, ident: &str) -> String {
    if doc.len() != 0 {
        return format!(
            "{}/// {}\n",
            ident,
            doc.to_string().replace("\\n", &format!("\n{ident}/// "))
        );
    }
    String::new()
}

fn pretty_tuple(vec: &VecM<ScSpecTypeDef, 12>) -> String {
    format!("({})", vec.iter().map(pretty_type).join(", "))
}
