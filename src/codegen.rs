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
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        let wasm_path_str = self.wasm.to_string_lossy();
        let contents = fs::read(&self.wasm)?;
        let h = Host::default();
        let vm = Vm::new(&h, [0; 32].into(), &contents)?;
        eprintln!("File: {}", wasm_path_str);
        if let Some(spec) = vm.custom_section("contractspecv0") {
            eprintln!("Contract Spec: {}", base64::encode(spec));
            let mut cursor = Cursor::new(spec);
            let specs = ScSpecEntry::read_xdr_iter(&mut cursor).collect::<Result<Vec<_>, _>>()?;
            let code = soroban_spec::generate_types(&specs, Some(&wasm_path_str));
            let code_raw = code.to_string();
            let code_fmt = match syn::parse2(code) {
                Ok(item) => prettyplease::unparse(&syn::File {
                    shebang: None,
                    attrs: vec![],
                    items: vec![item],
                }),
                Err(_) => code_raw,
            };
            println!("{}", code_fmt);
            Ok(())
        } else {
            eprintln!("Contract Spec: Not Found");
            Err(Error::ContractSpecNotFound)
        }
    }
}
