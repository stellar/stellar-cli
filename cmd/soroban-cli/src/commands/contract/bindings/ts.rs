use std::{fmt::Debug, path::PathBuf, println};

use clap::{command, Parser};
use soroban_spec::gen::ts;

use crate::wasm;

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    wasm: wasm::Args,

    /// where to place generated project
    #[arg(long)]
    root_dir: PathBuf,

    #[arg(long)]
    contract_name: String,

    #[arg(long, alias = "id")]
    contract_id: String,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("failed generate TS from file: {0}")]
    GenerateTSFromFile(ts::GenerateFromFileError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        let spec = self.wasm.parse().unwrap().spec;
        println!("here");
        let p: ts::boilerplate::Project = self.wasm.wasm.clone().try_into()?;
        println!("2");
        p.init(&self.contract_name, &self.contract_id, &spec)?;
        Ok(())
    }
}
