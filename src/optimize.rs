use clap::Parser;
use std::fmt::Debug;
use wasm_opt::{OptimizationError, OptimizationOptions};

#[derive(Parser, Debug)]
pub struct Cmd {
    /// WASM file to optimize
    #[clap(long, parse(from_os_str))]
    wasm: std::path::PathBuf,
    /// Path to write the optimized WASM file to (defaults to same location as --wasm)
    #[clap(long, parse(from_os_str))]
    wasm_out: Option<std::path::PathBuf>,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("reading file: {0}")]
    ReadingFile(std::io::Error),
    #[error("optimization error: {0}")]
    OptimizationError(OptimizationError),
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        let wasm_size = std::fs::metadata(&self.wasm)
            .map_err(Error::ReadingFile)?
            .len();

        println!(
            "Reading: {} ({} bytes)",
            self.wasm.to_string_lossy(),
            wasm_size
        );

        let wasm_out = self.wasm_out.as_ref().unwrap_or(&self.wasm);
        println!("Writing to: {}...", self.wasm.to_string_lossy());

        OptimizationOptions::new_optimize_for_size_aggressively()
            .run(&self.wasm, wasm_out)
            .map_err(Error::OptimizationError)?;

        let wasm_out_size = std::fs::metadata(&wasm_out)
            .map_err(Error::ReadingFile)?
            .len();
        println!(
            "Optimized: {} ({} bytes)",
            wasm_out.to_string_lossy(),
            wasm_out_size
        );

        Ok(())
    }
}
