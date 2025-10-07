use clap::{arg, Parser};
use std::{fmt::Debug, path::PathBuf};
#[cfg(feature = "additional-libs")]
use wasm_opt::{Feature, OptimizationError, OptimizationOptions};

#[cfg(feature = "additional-libs")]
use crate::commands::global;
use crate::wasm;

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    /// Path to one or more wasm binaries
    #[arg(long, num_args = 1.., required = true)]
    wasm: Vec<PathBuf>,

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

    #[error("--wasm-out cannot be used with --wasm option when passing multiple files")]
    MultipleFilesOutput,
}

impl Cmd {
    #[cfg(not(feature = "additional-libs"))]
    pub fn run(&self, _global_args: &global::Args) -> Result<(), Error> {
        Err(Error::Install)
    }

    #[cfg(feature = "additional-libs")]
    pub fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        use crate::print::Print;

        let print = Print::new(global_args.quiet);
        print
            .warnln("`stellar contract optimize` is deprecated and will be removed in the future. Use `stellar contract build --optimize` instead.");

        optimize(false, self.wasm.clone(), self.wasm_out.clone())
    }
}

#[cfg(feature = "additional-libs")]
pub fn optimize(
    quiet: bool,
    wasm: Vec<PathBuf>,
    wasm_out: Option<std::path::PathBuf>,
) -> Result<(), Error> {
    if wasm.len() > 1 && wasm_out.is_some() {
        return Err(Error::MultipleFilesOutput);
    }

    for wasm_path in &wasm {
        let wasm_arg = wasm::Args {
            wasm: wasm_path.into(),
        };

        if !quiet {
            println!(
                "Reading: {path} ({wasm_size} bytes)",
                path = wasm_arg.wasm.to_string_lossy(),
                wasm_size = wasm_arg.len()?
            );
        }

        let wasm_out = wasm_out.clone().unwrap_or_else(|| {
            let mut wasm_out = wasm_arg.wasm.clone();
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
            .run(&wasm_arg.wasm, &wasm_out)
            .map_err(Error::OptimizationError)?;

        if !quiet {
            println!(
                "Optimized: {path} ({size} bytes)",
                path = wasm_out.to_string_lossy(),
                size = wasm::len(&wasm_out)?
            );
        }
    }

    Ok(())
}
