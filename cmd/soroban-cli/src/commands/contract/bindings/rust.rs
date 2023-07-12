use std::fmt::Debug;

use clap::{command, Parser};
use soroban_spec_rust::{self, ToFormattedString};

use crate::wasm;

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    wasm: wasm::Args,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("generate rust from file: {0}")]
    GenerateRustFromFile(soroban_spec_rust::GenerateFromFileError),
    #[error("format rust error: {0}")]
    FormatRust(String),
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        let wasm_path_str = self.wasm.wasm.to_string_lossy();
        let code = soroban_spec_rust::generate_from_file(&wasm_path_str, None)
            .map_err(Error::GenerateRustFromFile)?;
        match code.to_formatted_string() {
            Ok(formatted) => {
                println!("{formatted}");
                Ok(())
            }
            Err(e) => {
                println!("{code}");
                Err(Error::FormatRust(e.to_string()))
            }
        }
    }
}
