use std::{
    io::{stdin, Read},
    path::PathBuf,
};

use soroban_env_host::xdr::ReadXdr;
use soroban_sdk::xdr::{Limits, Transaction, TransactionEnvelope};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to decode XDR from base64")]
    Base64Decode,
    #[error("failed to decode XDR from file: {0}")]
    FileDecode(PathBuf),
    #[error("failed to decode XDR from stdin")]
    StdinDecode,
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// XDR input, either base64 encoded or file path and stdin if neither is provided
#[derive(Debug, clap::Args, Clone)]
#[group(skip)]
pub struct Args {
    /// Base64 encoded XDR transaction
    #[arg(
        long = "xdr-base64",
        env = "STELLAR_TXN_XDR_BASE64",
        conflicts_with = "xdr_file"
    )]
    pub xdr_base64: Option<String>,
    //// File containing Binary encoded data
    #[arg(
        long = "xdr-file",
        env = "STELLAR_TXN_XDR_FILE",
        conflicts_with = "xdr_base64"
    )]
    pub xdr_file: Option<PathBuf>,
}

impl Args {
    pub fn xdr<T: ReadXdr>(&self) -> Result<T, Error> {
        match (self.xdr_base64.as_ref(), self.xdr_file.as_ref()) {
            (Some(xdr_base64), None) => {
                T::from_xdr_base64(xdr_base64, Limits::none()).map_err(|_| Error::Base64Decode)
            }
            (_, Some(xdr_file)) => T::from_xdr(std::fs::read(xdr_file)?, Limits::none())
                .map_err(|_| Error::FileDecode(xdr_file.clone())),

            _ => {
                let mut buf = String::new();
                let _ = stdin()
                    .read_to_string(&mut buf)
                    .map_err(|_| Error::StdinDecode)?;
                T::from_xdr_base64(buf.trim(), Limits::none()).map_err(|_| Error::StdinDecode)
            }
        }
    }

    pub fn txn(&self) -> Result<Transaction, Error> {
        self.xdr::<Transaction>()
    }

    pub fn txn_envelope(&self) -> Result<TransactionEnvelope, Error> {
        self.xdr::<TransactionEnvelope>()
    }
}
