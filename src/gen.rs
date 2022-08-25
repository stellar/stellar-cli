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
    GenerateRustFromFile(#[from] rust::GenerateFromFileError),
    #[error("format rust error: {0}")]
    FormatRust(#[from] syn::Error),
    #[error("host")]
    Host(#[from] HostError),
    #[error("xdr error: {0}")]
    Xdr(#[from] soroban_env_host::xdr::Error),
    #[error("io")]
    Io(#[from] io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
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
        let code = rust::generate_from_file(&wasm_path_str, None)?;
        let code_raw = code.to_string();
        let code_fmt = prettyplease::unparse(&syn::parse_file(&code_raw)?);
        println!("{}", code_fmt);
        Ok(())
    }

    pub fn generate_json(&self) -> Result<(), Error> {
        let contents = fs::read(&self.wasm)?;
        let h = Host::default();
        let vm = Vm::new(&h, [0; 32].into(), &contents)?;

        if let Some(spec) = vm.custom_section("contractspecv0") {
            let mut cursor = Cursor::new(spec);
            for spec_entry in ScSpecEntry::read_xdr_iter(&mut cursor) {
                let spec_json = json::Entry::try_from(&spec_entry?)?;
                println!("{}", serde_json::to_string(&spec_json)?);
            }
        } else {
            return Err(Error::ContractSpecNotFound);
        }
        Ok(())
    }
}
