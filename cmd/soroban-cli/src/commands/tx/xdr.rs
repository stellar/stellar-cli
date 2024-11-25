use std::{
    io::{stdin, Read},
    path::PathBuf,
};

use crate::xdr::{Limits, ReadXdr, Transaction, TransactionEnvelope, TransactionV1Envelope};

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
    #[error("only transaction v1 is supported")]
    OnlyTransactionV1Supported,
}

pub fn tx_envelope_from_stdin() -> Result<TransactionEnvelope, Error> {
    from_stdin()
}
pub fn from_stdin<T: ReadXdr>() -> Result<T, Error> {
    let mut buf = String::new();
    let _ = stdin()
        .read_to_string(&mut buf)
        .map_err(|_| Error::StdinDecode)?;
    T::from_xdr_base64(buf.trim(), Limits::none()).map_err(|_| Error::StdinDecode)
}

pub fn unwrap_envelope_v1(tx_env: TransactionEnvelope) -> Result<Transaction, Error> {
    let TransactionEnvelope::Tx(TransactionV1Envelope { tx, .. }) = tx_env else {
        return Err(Error::OnlyTransactionV1Supported);
    };
    Ok(tx)
}
