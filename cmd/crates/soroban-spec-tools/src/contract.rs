use base64::{engine::general_purpose::STANDARD as base64, Engine as _};
use std::{
    fmt::Display,
    io::{self, Cursor},
};

use stellar_xdr::{
    self as xdr, Limited, Limits, ReadXdr, ScEnvMetaEntry, ScEnvMetaEntryInterfaceVersion,
    ScMetaEntry, ScMetaV0, ScSpecEntry, ScSpecFunctionV0, ScSpecUdtEnumV0, ScSpecUdtErrorEnumV0,
    ScSpecUdtStructV0, ScSpecUdtUnionV0, StringM, WriteXdr,
};

/// Maximum recursion depth allowed when decoding contract spec/meta sections.
///
/// These sections come from attacker-authored contract WASM, and `ScSpecTypeDef`
/// is a recursive XDR type, so decoding with `Limits::none()` (depth `u32::MAX`)
/// lets a deeply-nested type definition exhaust the stack and abort the process.
/// 500 matches `soroban-env-host`'s `DEFAULT_XDR_RW_LIMITS`, so any spec the
/// network would accept still decodes, while deeper input fails with a clean
/// `DepthLimitExceeded` error instead of a crash.
const SPEC_XDR_DEPTH_LIMIT: u32 = 500;

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
        let mut env_meta: Option<Vec<u8>> = None;
        let mut meta: Option<Vec<u8>> = None;
        let mut spec: Option<Vec<u8>> = None;
        for payload in wasmparser::Parser::new(0).parse_all(bytes) {
            let payload = payload?;
            if let wasmparser::Payload::CustomSection(section) = payload {
                let out = match section.name() {
                    "contractenvmetav0" => &mut env_meta,
                    "contractmetav0" => &mut meta,
                    "contractspecv0" => &mut spec,
                    _ => continue,
                };

                if let Some(existing_data) = out {
                    let combined_data = [existing_data, section.data()].concat();
                    *out = Some(combined_data);
                } else {
                    *out = Some(section.data().to_vec());
                }
            }
        }

        let mut env_meta_base64 = None;
        let env_meta = if let Some(env_meta) = env_meta {
            env_meta_base64 = Some(base64.encode(&env_meta));
            let cursor = Cursor::new(env_meta);
            let mut read = Limited::new(cursor, Limits::depth(SPEC_XDR_DEPTH_LIMIT));
            ScEnvMetaEntry::read_xdr_iter(&mut read).collect::<Result<Vec<_>, xdr::Error>>()?
        } else {
            vec![]
        };

        let mut meta_base64 = None;
        let meta = if let Some(meta) = meta {
            meta_base64 = Some(base64.encode(&meta));
            let cursor = Cursor::new(meta);
            let mut depth_limit_read = Limited::new(cursor, Limits::depth(SPEC_XDR_DEPTH_LIMIT));
            ScMetaEntry::read_xdr_iter(&mut depth_limit_read)
                .collect::<Result<Vec<_>, xdr::Error>>()?
        } else {
            vec![]
        };

        let (spec_base64, spec) = if let Some(spec) = spec {
            let (spec_base64, spec) = Spec::spec_to_base64(&spec)?;
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
        let mut read = Limited::new(cursor, Limits::depth(SPEC_XDR_DEPTH_LIMIT));
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
                    ScEnvMetaEntry::ScEnvMetaKindInterfaceVersion(
                        ScEnvMetaEntryInterfaceVersion {
                            protocol,
                            pre_release,
                        },
                    ) => {
                        writeln!(f, " • Protocol Version: {protocol}")?;
                        if pre_release != &0 {
                            writeln!(f, " • Pre-release Version: {pre_release})")?;
                        }
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
                        writeln!(
                            f,
                            " • {}: {}",
                            sanitize(&key.to_utf8_string_lossy()),
                            sanitize(&val.to_utf8_string_lossy())
                        )?;
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
                    ScSpecEntry::EventV0(_) => {}
                }
            }
        } else {
            writeln!(f, "Contract Spec: None")?;
        }
        Ok(())
    }
}

fn write_func(f: &mut std::fmt::Formatter<'_>, func: &ScSpecFunctionV0) -> std::fmt::Result {
    writeln!(
        f,
        " • Function: {}",
        sanitize(&func.name.to_utf8_string_lossy())
    )?;
    if !func.doc.is_empty() {
        writeln!(
            f,
            "     Docs: {}",
            indent(&sanitize(&func.doc.to_utf8_string_lossy()), 11).trim()
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
    if !udt.doc.is_empty() {
        writeln!(
            f,
            "     Docs: {}",
            indent(&sanitize(&udt.doc.to_utf8_string_lossy()), 10).trim()
        )?;
    }
    writeln!(f, "     Cases:")?;
    for case in &udt.cases {
        writeln!(f, "      • {}", indent(&format!("{case:#?}"), 8).trim())?;
    }
    writeln!(f)?;
    Ok(())
}

fn write_struct(f: &mut std::fmt::Formatter<'_>, udt: &ScSpecUdtStructV0) -> std::fmt::Result {
    writeln!(f, " • Struct: {}", format_name(&udt.lib, &udt.name))?;
    if !udt.doc.is_empty() {
        writeln!(
            f,
            "     Docs: {}",
            indent(&sanitize(&udt.doc.to_utf8_string_lossy()), 10).trim()
        )?;
    }
    writeln!(f, "     Fields:")?;
    for field in &udt.fields {
        writeln!(
            f,
            "      • {}: {}",
            sanitize(&field.name.to_utf8_string_lossy()),
            indent(&format!("{:#?}", field.type_), 8).trim()
        )?;
        if !field.doc.is_empty() {
            writeln!(f, "{}", indent(&format!("{:#?}", field.doc), 8))?;
        }
    }
    writeln!(f)?;
    Ok(())
}

fn write_enum(f: &mut std::fmt::Formatter<'_>, udt: &ScSpecUdtEnumV0) -> std::fmt::Result {
    writeln!(f, " • Enum: {}", format_name(&udt.lib, &udt.name))?;
    if !udt.doc.is_empty() {
        writeln!(
            f,
            "     Docs: {}",
            indent(&sanitize(&udt.doc.to_utf8_string_lossy()), 10).trim()
        )?;
    }
    writeln!(f, "     Cases:")?;
    for case in &udt.cases {
        writeln!(f, "      • {}", indent(&format!("{case:#?}"), 8).trim())?;
    }
    writeln!(f)?;
    Ok(())
}

fn write_error(f: &mut std::fmt::Formatter<'_>, udt: &ScSpecUdtErrorEnumV0) -> std::fmt::Result {
    writeln!(f, " • Error: {}", format_name(&udt.lib, &udt.name))?;
    if !udt.doc.is_empty() {
        writeln!(
            f,
            "     Docs: {}",
            indent(&sanitize(&udt.doc.to_utf8_string_lossy()), 10).trim()
        )?;
    }
    writeln!(f, "     Cases:")?;
    for case in &udt.cases {
        writeln!(f, "      • {}", indent(&format!("{case:#?}"), 8).trim())?;
    }
    writeln!(f)?;
    Ok(())
}

pub fn sanitize(s: &str) -> String {
    escape_bytes::escape(s.as_bytes())
        .into_iter()
        .map(char::from)
        .collect()
}

fn indent(s: &str, n: usize) -> String {
    let pad = " ".repeat(n);
    s.lines()
        .map(|line| format!("{pad}{line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_name(lib: &StringM<80>, name: &StringM<60>) -> String {
    if lib.is_empty() {
        sanitize(&name.to_utf8_string_lossy())
    } else {
        format!(
            "{}::{}",
            sanitize(&lib.to_utf8_string_lossy()),
            sanitize(&name.to_utf8_string_lossy())
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::borrow::Cow;
    use stellar_xdr::{ScSpecFunctionV0, ScSpecTypeDef, ScSpecTypeOption};

    /// Wraps `spec` bytes in a minimal WASM module's `contractspecv0` section.
    fn wasm_with_spec(spec: &[u8]) -> Vec<u8> {
        let mut module = wasm_encoder::Module::new();
        module.section(&wasm_encoder::CustomSection {
            name: Cow::Borrowed("contractspecv0"),
            data: Cow::Borrowed(spec),
        });
        module.finish()
    }

    /// A spec entry for a function whose single output has type `type_`.
    fn fn_entry_returning(type_: ScSpecTypeDef) -> ScSpecEntry {
        ScSpecEntry::FunctionV0(ScSpecFunctionV0 {
            outputs: vec![type_].try_into().unwrap(),
            ..Default::default()
        })
    }

    /// A contract spec whose type nesting exceeds the decoder's depth limit must
    /// fail with a clean error rather than aborting the process via stack
    /// exhaustion. See `SPEC_XDR_DEPTH_LIMIT`.
    #[test]
    fn deeply_nested_spec_type_is_rejected() {
        // `ScSpecTypeDef::Option` boxes another `ScSpecTypeDef`, so decoding it
        // recurses once per level. Nest well past the limit.
        let mut type_ = ScSpecTypeDef::Bool;
        for _ in 0..(SPEC_XDR_DEPTH_LIMIT + 100) {
            type_ = ScSpecTypeDef::Option(Box::new(ScSpecTypeOption {
                value_type: Box::new(type_),
            }));
        }
        let entry = fn_entry_returning(type_);
        // Encode without limits so the crafted bytes reflect an attacker's WASM;
        // the guard under test is on the decode side.
        let spec = entry.to_xdr(Limits::none()).unwrap();
        let wasm = wasm_with_spec(&spec);

        match Spec::new(&wasm) {
            Err(Error::Xdr(xdr::Error::DepthLimitExceeded)) => {}
            Err(e) => panic!("expected DepthLimitExceeded, got error {e:?}"),
            Ok(_) => panic!("expected DepthLimitExceeded, but the spec decoded"),
        }
    }

    /// A normally-nested spec still decodes successfully under the depth limit.
    #[test]
    fn shallow_spec_type_is_accepted() {
        let type_ = ScSpecTypeDef::Option(Box::new(ScSpecTypeOption {
            value_type: Box::new(ScSpecTypeDef::Bool),
        }));
        let entry = fn_entry_returning(type_);
        let spec = entry.to_xdr(Limits::none()).unwrap();
        let wasm = wasm_with_spec(&spec);

        let parsed = Spec::new(&wasm).expect("shallow spec should decode");
        assert_eq!(parsed.spec, vec![entry]);
    }
}
