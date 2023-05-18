use std::{fmt::Debug, path::PathBuf};

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
        std::fs::remove_dir_all(&self.root_dir)?;
        std::fs::create_dir(&self.root_dir)?;
        let p: ts::boilerplate::Project = self.root_dir.clone().try_into()?;
        p.init(&self.contract_name, &self.contract_id, &spec)?;
        Ok(())
    }
}
