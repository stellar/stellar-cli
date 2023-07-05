use clap::{command, Parser};
use std::fmt::Debug;

use crate::{commands::config::locator, wasm};

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    wasm: wasm::Args,
    #[clap(flatten)]
    locator: locator::Args,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Wasm(#[from] wasm::Error),
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        println!("File: {}", self.wasm.wasm.to_string_lossy());
        println!("{}", self.wasm.parse()?);
        Ok(())
    }
}
