use clap::{command, Parser};
use std::fmt::Debug;

use crate::{commands::config::locator, wasm};

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    wasm: Option<wasm::Args>,
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
        if let Some(wasm) = &self.wasm {
            println!("File: {}", wasm.wasm.to_string_lossy());
            println!("{}", wasm.parse()?);
        } else {
        }
        Ok(())
    }
}
