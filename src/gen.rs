use std::fmt::Debug;

use clap::{ArgEnum, Parser};
use soroban_spec::gen::rust::{generate_from_file, GenerateFromFileError};

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
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("generate contract spec from file: {0}")]
    GenerateFromFile(GenerateFromFileError),
    #[error("parse for format error: {0}")]
    ParseForFormatError(syn::Error),
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        let wasm_path_str = self.wasm.to_string_lossy();
        let code = generate_from_file(&wasm_path_str, None).map_err(Error::GenerateFromFile)?;
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
