use std::{
    io::{stdin, Read},
    path::PathBuf,
};

use soroban_env_host::xdr::ReadXdr;
use soroban_sdk::xdr::{Limits, Transaction, TransactionEnvelope, TransactionV1Envelope};

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
    let buf = stdin_to_string()?;
    T::from_xdr_base64(buf.trim(), Limits::none()).map_err(|_| Error::StdinDecode)
}

pub fn stdin_to_string() -> Result<String, Error> {
    let mut buf = String::new();
    stdin().read_to_string(&mut buf)?;
    Ok(buf)
}

pub fn stdin_one_line() -> Result<String, Error> {
    let mut buf: [u8;1] = [0];
    stdin().read_exact(&mut buf)?;
    Ok(String::from_utf8(buf.to_vec()).unwrap())
}

pub fn unwrap_envelope_v1_from_stdin() -> Result<Transaction, Error> {
    unwrap_envelope_v1(tx_envelope_from_stdin()?)
}

pub fn unwrap_envelope_v1(tx_env: TransactionEnvelope) -> Result<Transaction, Error> {
    let TransactionEnvelope::Tx(TransactionV1Envelope { tx, .. }) = tx_env else {
        return Err(Error::OnlyTransactionV1Supported);
    };
    Ok(tx)
}
