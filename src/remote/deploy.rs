use std::{fmt::Debug, fs, io};

use clap::Parser;
use soroban_env_host::xdr::Error as XdrError;

use super::Remote;

#[derive(Parser, Debug)]
pub struct Cmd {
    /// WASM file to deploy
    #[clap(long, parse(from_os_str))]
    wasm: std::path::PathBuf,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("xdr processing error: {0}")]
    Xdr(#[from] XdrError),
    #[error("reading file {filepath}: {error}")]
    CannotReadContractFile {
        filepath: std::path::PathBuf,
        error: io::Error,
    },
}

impl Cmd {
    pub fn run(&self, _remote: &Remote) -> Result<(), Error> {
        let _contract = fs::read(&self.wasm).map_err(|e| Error::CannotReadContractFile {
            filepath: self.wasm.clone(),
            error: e,
        })?;

        // TODO: Call out to RPC or horizon to deploy.

        Ok(())
    }
}
