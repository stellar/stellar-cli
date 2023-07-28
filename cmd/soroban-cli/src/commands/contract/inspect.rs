use clap::{command, Parser};
use soroban_env_host::xdr::{self, WriteXdr};
use std::{fmt::Debug, path::PathBuf};
use tracing::{debug, trace};

use super::SpecOutput;
use crate::{commands::config::locator, wasm};

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    wasm: wasm::Args,
    /// Output just XDR in base64
    #[arg(long, default_value = "SpecOutput::Docs")]
    output: SpecOutput,

    #[clap(flatten)]
    locator: locator::Args,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Wasm(#[from] wasm::Error),
    #[error("missing spec for {0:?}")]
    MissingSpec(PathBuf),
    #[error(transparent)]
    Xdr(#[from] xdr::Error),
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        let wasm = self.wasm.parse()?;
        debug!("File: {}", self.wasm.wasm.to_string_lossy());
        let output = match self.output {
            SpecOutput::XdrBase64 => wasm
                .spec_base64
                .clone()
                .ok_or_else(|| Error::MissingSpec(self.wasm.wasm.clone()))?,
            SpecOutput::XdrBase64Array => {
                let spec = wasm
                    .spec
                    .iter()
                    .map(|e| {
                        trace!("{e:#?}\n{}\n", e.to_xdr_base64()?);
                        Ok(format!("\"{}\"", e.to_xdr_base64()?))
                    })
                    .collect::<Result<Vec<_>, Error>>()?
                    .join(",\n");
                format!("[{spec}]")
            }
            SpecOutput::Docs => wasm.to_string(),
        };
        println!("{output}");
        Ok(())
    }
}
