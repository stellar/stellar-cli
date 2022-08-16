use std::{
    fmt::Debug,
    fs,
    io::{self, Cursor},
};

use clap::Parser;
use soroban_env_host::{
    xdr::{self, ReadXdr, ScSpecEntry},
    Host, HostError, Vm,
};

#[derive(Parser, Debug)]
pub struct Cmd {
    /// WASM file to generating bindings for
    #[clap(long, parse(from_os_str))]
    wasm: std::path::PathBuf,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("xdr")]
    Xdr(#[from] xdr::Error),
    #[error("io")]
    Io(#[from] io::Error),
    #[error("host")]
    Host(#[from] HostError),
    #[error("contract spec not found")]
    ContractSpecNotFound,
    #[error("parse for format error")]
    ParseForFormatError(syn::Error),
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        let wasm_path_str = self.wasm.to_string_lossy();
        let contents = fs::read(&self.wasm)?;
        let h = Host::default();
        let vm = Vm::new(&h, [0; 32].into(), &contents)?;
        println!("// File: {}", wasm_path_str);
        if let Some(spec) = vm.custom_section("contractspecv0") {
            println!("// Contract Spec: {}", base64::encode(spec));
            let mut cursor = Cursor::new(spec);
            let specs = ScSpecEntry::read_xdr_iter(&mut cursor).collect::<Result<Vec<_>, _>>()?;
            let code = soroban_spec::generate(&specs, &contents);
            let code_raw = code.to_string();
            let (code_fmt, res) = match syn::parse_file(&code_raw) {
                Ok(file) => (prettyplease::unparse(&file), Ok(())),
                Err(e) => (code_raw, Err(Error::ParseForFormatError(e))),
            };
            println!("{}", code_fmt);
            res
        } else {
            println!("// Contract Spec: Not Found");
            Err(Error::ContractSpecNotFound)
        }
    }
}
