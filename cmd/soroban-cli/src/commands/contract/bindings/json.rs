use std::fmt::Debug;

use clap::{command, Parser};
use soroban_spec_json;

use crate::wasm;

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    wasm: wasm::Args,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("generate json from file: {0}")]
    GenerateJsonFromFile(soroban_spec_json::GenerateFromFileError),
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        let wasm_path_str = self.wasm.wasm.to_string_lossy();
        let json = soroban_spec_json::generate_from_file(&wasm_path_str, None)
            .map_err(Error::GenerateJsonFromFile)?;
        println!("{json}");
        Ok(())
    }
}
