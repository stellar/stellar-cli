use clap::{command, Parser};
use std::fmt::Debug;

use crate::wasm;

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    wasm: wasm::Args,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Wasm(#[from] wasm::Error),
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        println!("File: {}", self.wasm.wasm.to_string_lossy());
        let parsed = self.wasm.parse()?;
        if parsed.env_meta.len() > 0 {
            print!("{:#?}", (parsed.env_meta, parsed.spec.clone()));
        } else {
            print!("{:#?}", (parsed.spec));
        }
        Ok(())
    }
}
