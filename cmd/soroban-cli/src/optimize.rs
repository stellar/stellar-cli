use clap::Parser;
use std::fmt::Debug;
use wasm_opt::{OptimizationError, OptimizationOptions};

#[derive(Parser, Debug)]
pub struct Cmd {
    /// WASM file to optimize
    #[clap(long, parse(from_os_str))]
    wasm: std::path::PathBuf,
    /// Path to write the optimized WASM file to (defaults to same location as --wasm with .optimized.wasm suffix)
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

        let wasm_out = self.wasm_out.as_ref().cloned().unwrap_or_else(|| {
            let mut wasm_out = self.wasm.clone();
            wasm_out.set_extension("optimized.wasm");
            wasm_out
        });
        println!("Writing to: {}...", self.wasm.to_string_lossy());

        let mut options = OptimizationOptions::new_optimize_for_size_aggressively();
        options.converge = true;

        // Don't let wasm-opt use any optional features,
        // including the default signext, and mutable globals.
        // Soroban disables all optional wasm features in wasmi.
        options.mvp_features_only();

        options
            .run(&self.wasm, &wasm_out)
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
