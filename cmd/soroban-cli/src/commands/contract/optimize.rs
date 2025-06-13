use clap::{arg, command, Parser};
use std::fmt::Debug;
#[cfg(feature = "additional-libs")]
use wasm_opt::{Feature, OptimizationError, OptimizationOptions};

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
    #[cfg(feature = "additional-libs")]
    #[error("optimization error: {0}")]
    OptimizationError(OptimizationError),
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

        let mut options = OptimizationOptions::new_optimize_for_size_aggressively();
        options.converge = true;

        // Explicitly set to MVP + sign-ext + mutable-globals, which happens to
        // also be the default featureset, but just to be extra clear we set it
        // explicitly.
        //
        // Formerly Soroban supported only the MVP feature set, but Rust 1.70 as
        // well as Clang generate code with sign-ext + mutable-globals enabled,
        // so Soroban has taken a change to support them also.
        options.mvp_features_only();
        options.enable_feature(Feature::MutableGlobals);
        options.enable_feature(Feature::SignExt);

        options
            .run(&self.wasm.wasm, &wasm_out)
            .map_err(Error::OptimizationError)?;

        let wasm_out_size = wasm::len(&wasm_out)?;
        println!(
            "Optimized: {} ({} bytes)",
            wasm_out.to_string_lossy(),
            wasm_out_size
        );

        Ok(())
    }
}
