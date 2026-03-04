use clap::Parser;
use std::fmt::Debug;

use crate::{commands::global, print::Print, wasm};

/// Verify that a contract's spec references only defined types
///
/// Reads a contract WASM and checks that all user-defined types (UDTs)
/// referenced in function signatures, events, and type definitions are
/// defined within the spec itself.
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
    #[error(transparent)]
    Spec(#[from] soroban_spec_tools::Error),
    #[error("contract spec has undefined types")]
    VerifyFailed,
}

impl Cmd {
    pub fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let print = Print::new(global_args.quiet);
        let wasm_bytes = self.wasm.read()?;
        let spec = soroban_spec_tools::Spec::from_wasm(&wasm_bytes)?;
        let warnings = spec.verify();
        if warnings.is_empty() {
            print.checkln("contract spec verification passed: all types are defined");
        } else {
            for w in &warnings {
                print.warnln(w);
            }
            return Err(Error::VerifyFailed);
        }
        Ok(())
    }
}
