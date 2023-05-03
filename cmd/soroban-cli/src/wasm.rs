use clap::arg;
use std::{
    fmt::Display,
    fs,
    io::{self, Cursor},
    path::Path,
};

use soroban_env_host::xdr::{self, ReadXdr, ScEnvMetaEntry, ScSpecEntry, ScMetaEntry, ScMetaV0};

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
        } else {
            writeln!(f, "Env Meta: None")?;
        }

        if let Some(meta) = &self.meta_base64 {
            writeln!(f, "Contract Meta: {meta}")?;
            for meta_entry in &self.meta {
                match meta_entry {
                    ScMetaEntry::ScMetaV0(ScMetaV0{key, val}) => {
                        writeln!(f, " • {key}: {val}")?;
                    }
                }
            }
        } else {
            writeln!(f, "Contract Meta: None")?;
        }

        if let Some(spec_base64) = &self.spec_base64 {
            writeln!(f, "Contract Spec: {spec_base64}")?;
            for spec_entry in &self.spec {
                match spec_entry {
                    ScSpecEntry::FunctionV0(func) => writeln!(
                        f,
                        " • Function: {}\n   Inputs: ({:?})\n   Returns: ({:?}){}\n",
                        func.name.to_string_lossy(),
                        func.inputs.as_slice(),
                        func.outputs.as_slice(),
                        if func.doc.len() > 0 {
                            "\n   Docs: ".to_owned()+&func.doc.to_string_lossy().replace("\n", "\n         ")
                        } else {
                            "".to_string()
                        }
                    )?,
                    ScSpecEntry::UdtUnionV0(udt) => {
                        writeln!(f, " • Union: {udt:?}\n")?;
                    }
                    ScSpecEntry::UdtStructV0(udt) => {
                        writeln!(f, " • Struct: {udt:?}\n")?;
                    }
                    ScSpecEntry::UdtEnumV0(udt) => {
                        writeln!(f, " • Enum: {udt:?}\n")?;
                    }
                    ScSpecEntry::UdtErrorEnumV0(udt) => {
                        writeln!(f, " • Error: {udt:?}\n")?;
                    }
                }
            }
        } else {
            writeln!(f, "Contract Spec: None")?;
        }
        Ok(())
    }
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
