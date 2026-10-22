use std::fmt::Debug;

use clap::Parser;
use soroban_spec_rust::{self, ToFormattedString};
use soroban_spec_tools::sanitize;

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
    GenerateRustFromFile(String),
    #[error("format rust error: {0}")]
    FormatRust(String),
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        let wasm_path_str = self.wasm.wasm.to_string_lossy();
        // The error can name entries of the spec, so sanitize it before it is
        // printed, as the spec is untrusted input.
        let code = soroban_spec_rust::generate_from_file(&wasm_path_str, None)
            .map_err(|e| Error::GenerateRustFromFile(sanitize(&e.to_string())))?;
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
