use std::fmt::Debug;

use clap::Parser;
use soroban_spec::GenerateFromFileError;

#[derive(Parser, Debug)]
pub struct Cmd {
    /// WASM file to generating bindings for
    #[clap(long, parse(from_os_str))]
    wasm: std::path::PathBuf,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("generate contract spec from file")]
    GenerateFromFile(GenerateFromFileError),
    #[error("parse for format error")]
    ParseForFormatError(syn::Error),
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        let wasm_path_str = self.wasm.to_string_lossy();
        let code =
            soroban_spec::generate_from_file(&wasm_path_str, None).map_err(Error::GenerateFromFile)?;
        let code_raw = code.to_string();
        match syn::parse_file(&code_raw) {
            Ok(file) => {
                let code_fmt = prettyplease::unparse(&file);
                println!("{}", code_fmt);
                Ok(())
            }
            Err(e) => {
                println!("{}", code_raw);
                Err(Error::ParseForFormatError(e))
            }
        }
    }
}
