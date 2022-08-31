use std::fmt::Debug;

use clap::{ArgEnum, Parser};
use soroban_spec::gen::{json, rust};

use crate::error;

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

impl Cmd {
    pub fn run(&self) -> Result<(), error::Cmd> {
        match self.output {
            Output::Rust => self.generate_rust(),
            Output::Json => self.generate_json(),
        }
    }

    pub fn generate_rust(&self) -> Result<(), error::Cmd> {
        let wasm_path_str = self.wasm.to_string_lossy();
        let code = rust::generate_from_file(&wasm_path_str, None)
            .map_err(error::Cmd::CannotGenerateRustFromFile)?;
        let code_raw = code.to_string();
        match syn::parse_file(&code_raw) {
            Ok(file) => {
                let code_fmt = prettyplease::unparse(&file);
                println!("{}", code_fmt);
                Ok(())
            }
            Err(e) => {
                println!("{}", code_raw);
                Err(error::Cmd::CannotFormatRust(e))
            }
        }
    }

    pub fn generate_json(&self) -> Result<(), error::Cmd> {
        let wasm_path_str = self.wasm.to_string_lossy();
        let json = json::generate_from_file(&wasm_path_str, None)
            .map_err(error::Cmd::CannotGenerateJsonFromFile)?;
        println!("{}", json);
        Ok(())
    }
}
