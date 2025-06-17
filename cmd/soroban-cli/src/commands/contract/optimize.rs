use clap::{arg, command, Parser};
use std::fmt::Debug;

use crate::wasm;

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    wasm: wasm::Args,
    /// Path to write the optimized WASM file to (defaults to same location as --wasm with .optimized.wasm suffix)
    #[arg(long)]
    wasm_out: Option<std::path::PathBuf>,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Wasm(#[from] wasm::Error),
    #[cfg(not(feature = "additional-libs"))]
    #[error("must install with \"additional-libs\" feature.")]
    Install,
}

impl Cmd {
    #[cfg(not(feature = "additional-libs"))]
    pub fn run(&self) -> Result<(), Error> {
        Err(Error::Install)
    }

    #[cfg(feature = "additional-libs")]
    pub fn run(&self) -> Result<(), Error> {
        let wasm_size = self.wasm.len()?;

        println!(
            "Reading: {} ({} bytes)",
            self.wasm.wasm.to_string_lossy(),
            wasm_size
        );

        let wasm_out = self.wasm_out.clone().unwrap_or_else(|| {
            let mut wasm_out = self.wasm.wasm.clone();
            wasm_out.set_extension("optimized.wasm");
            wasm_out
        });

        self.wasm.optimize(&wasm_out)?;

        let wasm_out_size = wasm::len(&wasm_out)?;
        println!(
            "Optimized: {} ({} bytes)",
            wasm_out.to_string_lossy(),
            wasm_out_size
        );

        Ok(())
    }
}
