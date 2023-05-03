use clap::arg;
use std::{
    fmt::Display,
    fs,
    io::{self, Cursor},
    path::Path,
};

use soroban_env_host::xdr::{self, ReadXdr, ScEnvMetaEntry, ScMetaEntry, ScMetaV0, ScSpecEntry};

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
}

#[derive(Debug, clap::Args, Clone)]
#[group(skip)]
pub struct Args {
    /// Path to wasm binary
    #[arg(long)]
    pub wasm: std::path::PathBuf,
}

impl Args {
    /// # Errors
    /// May fail to read wasm file
    pub fn read(&self) -> Result<Vec<u8>, Error> {
        fs::read(&self.wasm).map_err(|e| Error::CannotReadContractFile {
            filepath: self.wasm.clone(),
            error: e,
        })
    }

    /// # Errors
    /// May fail to read wasm file
    pub fn len(&self) -> Result<u64, Error> {
        len(&self.wasm)
    }

    /// # Errors
    /// May fail to read wasm file
    pub fn is_empty(&self) -> Result<bool, Error> {
        self.len().map(|len| len == 0)
    }

    /// # Errors
    /// May fail to read wasm file or parse xdr section
    pub fn parse(&self) -> Result<ContractSpec, Error> {
        let contents = self.read()?;
        let mut env_meta: Option<&[u8]> = None;
        let mut meta: Option<&[u8]> = None;
        let mut spec: Option<&[u8]> = None;
        for payload in wasmparser::Parser::new(0).parse_all(&contents) {
            let payload = payload.map_err(|e| Error::CannotParseWasm {
                file: self.wasm.clone(),
                error: e,
            })?;
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
            env_meta_base64 = Some(base64::encode(env_meta));
            let mut cursor = Cursor::new(env_meta);
            ScEnvMetaEntry::read_xdr_iter(&mut cursor).collect::<Result<Vec<_>, xdr::Error>>()?
        } else {
            vec![]
        };

        let mut meta_base64 = None;
        let meta = if let Some(meta) = meta {
            meta_base64 = Some(base64::encode(meta));
            let mut cursor = Cursor::new(meta);
            ScMetaEntry::read_xdr_iter(&mut cursor).collect::<Result<Vec<_>, xdr::Error>>()?
        } else {
            vec![]
        };

        let mut spec_base64 = None;
        let spec = if let Some(spec) = spec {
            spec_base64 = Some(base64::encode(spec));
            let mut cursor = Cursor::new(spec);
            ScSpecEntry::read_xdr_iter(&mut cursor).collect::<Result<Vec<_>, xdr::Error>>()?
        } else {
            vec![]
        };

        Ok(ContractSpec {
            env_meta_base64,
            env_meta,
            meta_base64,
            meta,
            spec_base64,
            spec,
        })
    }
}

pub struct ContractSpec {
    pub env_meta_base64: Option<String>,
    pub env_meta: Vec<ScEnvMetaEntry>,
    pub meta_base64: Option<String>,
    pub meta: Vec<ScMetaEntry>,
    pub spec_base64: Option<String>,
    pub spec: Vec<ScSpecEntry>,
}

impl Display for ContractSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(env_meta) = &self.env_meta_base64 {
            writeln!(f, "Env Meta: {env_meta}")?;
            for env_meta_entry in &self.env_meta {
                match env_meta_entry {
                    ScEnvMetaEntry::ScEnvMetaKindInterfaceVersion(v) => {
                        writeln!(f, " • Interface Version: {v}")?;
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
                    ScSpecEntry::FunctionV0(func) => {
                        writeln!(f, " • Function: {}", func.name.to_string_lossy())?;
                        if func.doc.len() > 0 {
                            writeln!(
                                f,
                                "     Docs: {}",
                                &indent(&func.doc.to_string_lossy(), 11).trim()
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
                    }
                    ScSpecEntry::UdtUnionV0(udt) => {
                        // TODO: What is `lib`? Do we need that?
                        writeln!(f, " • Union: {}", udt.name.to_string_lossy())?;
                        if udt.doc.len() > 0 {
                            writeln!(
                                f,
                                "     Docs: {}",
                                indent(&udt.doc.to_string_lossy(), 10).trim()
                            )?;
                        }
                        writeln!(f, "     Cases:")?;
                        for case in udt.cases.iter() {
                            writeln!(f, "      • {}", indent(&format!("{:#?}", case), 8).trim())?;
                        }
                        writeln!(f)?;
                    }
                    ScSpecEntry::UdtStructV0(udt) => {
                        // TODO: What is `lib`? Do we need that?
                        writeln!(f, " • Struct: {}", udt.name.to_string_lossy())?;
                        if udt.doc.len() > 0 {
                            writeln!(
                                f,
                                "     Docs: {}",
                                indent(&udt.doc.to_string_lossy(), 10).trim()
                            )?;
                        }
                        writeln!(f, "     Fields:")?;
                        for field in udt.fields.iter() {
                            writeln!(
                                f,
                                "      • {}: {}",
                                field.name.to_string_lossy(),
                                indent(&format!("{:#?}", field.type_), 8).trim()
                            )?;
                            if field.doc.len() > 0 {
                                writeln!(f, "{}", indent(&format!("{:#?}", field.doc), 8))?;
                            }
                        }
                        writeln!(f)?;
                    }
                    ScSpecEntry::UdtEnumV0(udt) => {
                        // TODO: What is `lib`? Do we need that?
                        writeln!(f, " • Enum: {}", udt.name.to_string_lossy())?;
                        if udt.doc.len() > 0 {
                            writeln!(
                                f,
                                "     Docs: {}",
                                indent(&udt.doc.to_string_lossy(), 10).trim()
                            )?;
                        }
                        writeln!(f, "     Cases:")?;
                        for case in udt.cases.iter() {
                            writeln!(f, "      • {}", indent(&format!("{:#?}", case), 8).trim())?;
                        }
                        writeln!(f)?;
                    }
                    ScSpecEntry::UdtErrorEnumV0(udt) => {
                        // TODO: What is `lib`? Do we need that?
                        writeln!(f, " • Error: {}", udt.name.to_string_lossy())?;
                        if udt.doc.len() > 0 {
                            writeln!(
                                f,
                                "     Docs: {}",
                                indent(&udt.doc.to_string_lossy(), 10).trim()
                            )?;
                        }
                        writeln!(f, "     Cases:")?;
                        for case in udt.cases.iter() {
                            writeln!(f, "      • {}", indent(&format!("{:#?}", case), 8).trim())?;
                        }
                        writeln!(f)?;
                    }
                }
            }
        } else {
            writeln!(f, "Contract Spec: None")?;
        }
        Ok(())
    }
}

fn indent(s: &str, n: usize) -> String {
    let pad = " ".repeat(n);
    s.lines()
        .map(|line| format!("{}{}", pad, line))
        .collect::<Vec<_>>()
        .join("\n")
}

/// # Errors
/// May fail to read wasm file
pub fn len(p: &Path) -> Result<u64, Error> {
    Ok(std::fs::metadata(p)
        .map_err(|e| Error::CannotReadContractFile {
            filepath: p.to_path_buf(),
            error: e,
        })?
        .len())
}
