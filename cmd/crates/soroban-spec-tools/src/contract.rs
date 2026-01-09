use base64::{engine::general_purpose::STANDARD as base64, Engine as _};
use std::{
    fmt::Display,
    io::{self, Cursor},
};

use stellar_xdr::curr::{
    self as xdr, Limited, Limits, ReadXdr, ScEnvMetaEntry, ScEnvMetaEntryInterfaceVersion,
    ScMetaEntry, ScMetaV0, ScSpecEntry, ScSpecFunctionV0, ScSpecUdtEnumV0, ScSpecUdtErrorEnumV0,
    ScSpecUdtStructV0, ScSpecUdtUnionV0, StringM, WriteXdr,
};

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
            let mut read = Limited::new(cursor, Limits::none());
            ScEnvMetaEntry::read_xdr_iter(&mut read).collect::<Result<Vec<_>, xdr::Error>>()?
        } else {
            vec![]
        };

        let mut meta_base64 = None;
        let meta = if let Some(meta) = meta {
            meta_base64 = Some(base64.encode(&meta));
            let cursor = Cursor::new(meta);
            let mut depth_limit_read = Limited::new(cursor, Limits::none());
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
        let mut read = Limited::new(cursor, Limits::none());
        Ok((
            spec_base64,
            ScSpecEntry::read_xdr_iter(&mut read).collect::<Result<Vec<_>, xdr::Error>>()?,
        ))
    }

    /// Returns a filtered version of the spec with unused types removed.
    ///
    /// This removes any type definitions that are not referenced (directly or
    /// transitively) by any function in the contract. Functions and events are
    /// always preserved.
    #[must_use]
    pub fn filter_unused_types(&self) -> Vec<ScSpecEntry> {
        crate::filter::filter_unused_types(self.spec.clone())
    }

    /// Returns the filtered spec entries serialized as XDR bytes.
    ///
    /// This is useful for replacing the contractspecv0 custom section in a WASM
    /// file with a smaller version that only contains used types.
    pub fn filtered_spec_xdr(&self) -> Result<Vec<u8>, Error> {
        let filtered = self.filter_unused_types();
        let mut buffer = Vec::new();
        let mut writer = Limited::new(Cursor::new(&mut buffer), Limits::none());
        for entry in filtered {
            entry.write_xdr(&mut writer)?;
        }
        Ok(buffer)
    }

    /// Returns the filtered spec entries serialized as XDR bytes, filtering
    /// based on markers in the WASM data section.
    ///
    /// The SDK embeds markers in the data section for each type/event that is
    /// actually used in the contract. These markers survive dead code elimination,
    /// so we can filter out any spec entries that don't have corresponding markers.
    ///
    /// Functions are always kept as they define the contract's API.
    ///
    /// # Arguments
    ///
    /// * `wasm_bytes` - The WASM binary to extract markers from
    ///
    /// # Returns
    ///
    /// XDR bytes of the filtered spec entries.
    pub fn filtered_spec_xdr_with_markers(&self, wasm_bytes: &[u8]) -> Result<Vec<u8>, Error> {
        use crate::filter::{extract_spec_markers, filter_by_markers};

        // Extract markers from the WASM data section
        let markers = extract_spec_markers(wasm_bytes);

        // Filter all entries (types, events) based on markers
        let filtered = filter_by_markers(self.spec.clone(), &markers);

        let mut buffer = Vec::new();
        let mut writer = Limited::new(Cursor::new(&mut buffer), Limits::none());
        for entry in filtered {
            entry.write_xdr(&mut writer)?;
        }
        Ok(buffer)
    }
}

/// Replaces a custom section in WASM bytes with new content.
///
/// This function parses the WASM to find the target custom section, then rebuilds
/// the WASM by copying all other sections verbatim and appending the new custom
/// section at the end.
///
/// # Arguments
///
/// * `wasm_bytes` - The original WASM binary
/// * `section_name` - The name of the custom section to replace
/// * `new_content` - The new content for the custom section
///
/// # Returns
///
/// A new WASM binary with the custom section replaced.
pub fn replace_custom_section(
    wasm_bytes: &[u8],
    section_name: &str,
    new_content: &[u8],
) -> Result<Vec<u8>, Error> {
    use wasm_encoder::{CustomSection, Module, RawSection};
    use wasmparser::Payload;

    let mut module = Module::new();

    let parser = wasmparser::Parser::new(0);
    for payload in parser.parse_all(wasm_bytes) {
        let payload = payload?;

        // Skip the target custom section - we'll append the new one at the end
        let is_target_section =
            matches!(&payload, Payload::CustomSection(section) if section.name() == section_name);
        if !is_target_section {
            // For all other payloads that represent sections, copy them verbatim
            if let Some((id, range)) = payload.as_section() {
                let raw = RawSection {
                    id,
                    data: &wasm_bytes[range],
                };
                module.section(&raw);
            }
        }
    }

    // Append the new custom section
    let custom = CustomSection {
        name: section_name.into(),
        data: new_content.into(),
    };
    module.section(&custom);

    Ok(module.finish())
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
    writeln!(f, " • Function: {}", func.name.to_utf8_string_lossy())?;
    if !func.doc.is_empty() {
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
    if !udt.doc.is_empty() {
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
    if !udt.doc.is_empty() {
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
    if !udt.doc.is_empty() {
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
    if lib.is_empty() {
        name.to_utf8_string_lossy()
    } else {
        format!(
            "{}::{}",
            lib.to_utf8_string_lossy(),
            name.to_utf8_string_lossy()
        )
    }
}
