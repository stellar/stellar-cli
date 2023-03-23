use std::fmt::Debug;

use clap::{arg, command, Parser, ValueEnum};
use soroban_spec::gen::{
    json,
    rust::{self, ToFormattedString},
};

use crate::wasm;

#[derive(Parser, Debug)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    wasm: wasm::Args,
    /// Type of output to generate
    #[arg(long, value_enum)]
    r#output: Output,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, ValueEnum)]
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
    FormatRust(String),
    #[error("generate json from file: {0}")]
    GenerateJsonFromFile(json::GenerateFromFileError),
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        match self.output {
            Output::Rust => self.generate_rust(),
            Output::Json => self.generate_json(),
        }
    }

    pub fn generate_rust(&self) -> Result<(), Error> {
        let wasm_path_str = self.wasm.wasm.to_string_lossy();
        let code =
            rust::generate_from_file(&wasm_path_str, None).map_err(Error::GenerateRustFromFile)?;
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

    pub fn generate_json(&self) -> Result<(), Error> {
        let wasm_path_str = self.wasm.wasm.to_string_lossy();
        let json =
            json::generate_from_file(&wasm_path_str, None).map_err(Error::GenerateJsonFromFile)?;
        println!("{json}");
        Ok(())
    }
}
