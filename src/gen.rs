use std::{fmt::Debug, fs, io, io::Cursor};

use clap::{ArgEnum, Parser};
use soroban_env_host::{
    xdr::{ReadXdr, ScSpecEntry},
    Host, HostError, Vm,
};
use soroban_spec::gen::{json, rust};

#[derive(Parser, Debug)]
pub struct Cmd {
    /// WASM file to generate code for
    #[clap(long, parse(from_os_str))]
    wasm: std::path::PathBuf,
    /// Type of output to generate
    #[clap(long, arg_enum)]
    r#output: Output,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, ArgEnum)]
pub enum Output {
    /// Rust trait, client bindings, and test harness
    Rust,
    /// Json representation of contract spec types
    Json,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("generate rust from file: {0}")]
    GenerateRustFromFile(rust::GenerateFromFileError),
    #[error("format rust error: {0}")]
    FormatRust(syn::Error),
    #[error("host")]
    Host(HostError),
    #[error("xdr error: {0}")]
    Xdr(soroban_env_host::xdr::Error),
    #[error("io")]
    Io(io::Error),
    #[error("serialize json error: {0}")]
    Json(serde_json::Error),
    #[error("contractnotfound")]
    ContractSpecNotFound,
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        match self.output {
            Output::Rust => self.generate_rust(),
            Output::Json => self.generate_json(),
        }
    }

    pub fn generate_rust(&self) -> Result<(), Error> {
        let wasm_path_str = self.wasm.to_string_lossy();
        let code =
            rust::generate_from_file(&wasm_path_str, None).map_err(Error::GenerateRustFromFile)?;
        let code_raw = code.to_string();
        match syn::parse_file(&code_raw) {
            Ok(file) => {
                let code_fmt = prettyplease::unparse(&file);
                println!("{}", code_fmt);
                Ok(())
            }
            Err(e) => {
                println!("{}", code_raw);
                Err(Error::FormatRust(e))
            }
        }
    }

    pub fn generate_json(&self) -> Result<(), Error> {
        let contents = fs::read(&self.wasm).map_err(Error::Io)?;
        let h = Host::default();
        let vm = Vm::new(&h, [0; 32].into(), &contents).map_err(Error::Host)?;

        if let Some(spec) = vm.custom_section("contractspecv0") {
            let mut cursor = Cursor::new(spec);
            let json_entries: Result<Vec<json::Entry>, Error> =
                ScSpecEntry::read_xdr_iter(&mut cursor)
                    .map(|spec_entry| {
                        json::Entry::try_from(&spec_entry.map_err(Error::Xdr)?).map_err(Error::Xdr)
                    })
                    .collect();
            println!(
                "{}",
                serde_json::to_string(&json_entries?).map_err(Error::Json)?
            );
        } else {
            return Err(Error::ContractSpecNotFound);
        }
        Ok(())
    }
}
