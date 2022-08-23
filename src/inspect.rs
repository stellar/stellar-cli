use clap::Parser;
use soroban_env_host::{
    xdr::{self, ReadXdr, ScEnvMetaEntry, ScSpecEntry},
    HostError,
};
use std::{fmt::Debug, fs, io, io::Cursor, str::Utf8Error};

#[derive(Parser, Debug)]
pub struct Cmd {
    /// WASM file to inspect
    #[clap(long, parse(from_os_str))]
    wasm: std::path::PathBuf,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("parse wasm")]
    WasmParse(wasmparser::BinaryReaderError),
    #[error("xdr")]
    Xdr(#[from] xdr::Error),
    #[error("io")]
    Io(#[from] io::Error),
    #[error("host")]
    Host(#[from] HostError),
    #[error("utf8")]
    Utf8Error(#[from] Utf8Error),
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        println!("File: {}", self.wasm.to_string_lossy());

        let contents = fs::read(&self.wasm)?;

        let mut spec: Option<&[u8]> = None;
        let mut env_meta: Option<&[u8]> = None;
        for payload in wasmparser::Parser::new(0).parse_all(&contents) {
            let payload = payload.map_err(Error::WasmParse)?;
            if let wasmparser::Payload::CustomSection(section) = payload {
                let out = match section.name() {
                    "contractenvmetav0" => &mut env_meta,
                    "contractspecv0" => &mut spec,
                    _ => continue,
                };
                *out = Some(section.data());
            };
        }

        if let Some(env_meta) = env_meta {
            println!("Env Meta: {}", base64::encode(env_meta));
            let mut cursor = Cursor::new(env_meta);
            for env_meta_entry in ScEnvMetaEntry::read_xdr_iter(&mut cursor) {
                match env_meta_entry? {
                    ScEnvMetaEntry::ScEnvMetaKindInterfaceVersion(v) => {
                        println!(" • Interface Version: {}", v);
                    }
                }
            }
        } else {
            println!("Env Meta: None");
        }

        if let Some(spec) = spec {
            println!("Contract Spec: {}", base64::encode(spec));
            let mut cursor = Cursor::new(spec);
            for spec_entry in ScSpecEntry::read_xdr_iter(&mut cursor) {
                match spec_entry? {
                    ScSpecEntry::FunctionV0(f) => println!(
                        " • Function: {} ({:?}) -> ({:?})",
                        f.name.to_string()?,
                        f.inputs.as_slice(),
                        f.outputs.as_slice(),
                    ),
                    ScSpecEntry::UdtUnionV0(udt) => {
                        println!(" • Union: {:?}", udt);
                    }
                    ScSpecEntry::UdtStructV0(udt) => {
                        println!(" • Struct: {:?}", udt);
                    }
                }
            }
        } else {
            println!("Contract Spec: None");
        }
        Ok(())
    }
}
