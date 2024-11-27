use std::fmt::Debug;

use clap::Parser;
use soroban_spec_json;

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("generate json from file: {0}")]
    GenerateJsonFromFile(soroban_spec_json::GenerateFromFileError),
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        println!("Python bindings are provided by external library: https://github.com/lightsail-network/stellar-contract-bindings");
        Ok(())
    }
}
