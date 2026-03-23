use crate::xdr::{
    Limits, Operation, ReadXdr, Transaction, TransactionEnvelope, TransactionV1Envelope,
};
use std::ffi::OsString;
use std::fs::File;
use std::io::{stdin, Read};
use std::io::{Cursor, IsTerminal};
use std::path::Path;
use stellar_xdr::curr::{Limited, SkipWhitespace};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to decode XDR: {0}")]
    XDRDecode(#[from] stellar_xdr::curr::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("only transaction v1 is supported")]
    OnlyTransactionV1Supported,
    #[error("too many operations, limited to 100 operations in a transaction")]
    TooManyOperations,
    #[error("no transaction provided")]
    NoStdin,
}

pub fn tx_envelope_from_input(input: &Option<OsString>) -> Result<TransactionEnvelope, Error> {
    let read: &mut dyn Read = if let Some(input) = input {
        let exist = Path::new(input).try_exists();
        if let Ok(true) = exist {
            &mut File::open(input)?
        } else {
            &mut Cursor::new(input.clone().into_encoded_bytes())
        }
    } else {
        if stdin().is_terminal() {
            return Err(Error::NoStdin);
        }
        &mut stdin()
    };

    let mut lim = Limited::new(SkipWhitespace::new(read), Limits::none());
    Ok(TransactionEnvelope::read_xdr_base64_to_end(&mut lim)?)
}

pub fn unwrap_envelope_v1(tx_env: TransactionEnvelope) -> Result<Transaction, Error> {
    let TransactionEnvelope::Tx(TransactionV1Envelope { tx, .. }) = tx_env else {
        return Err(Error::OnlyTransactionV1Supported);
    };
    Ok(tx)
}

pub fn add_op(tx_env: TransactionEnvelope, op: Operation) -> Result<TransactionEnvelope, Error> {
    let mut tx = unwrap_envelope_v1(tx_env)?;
    let mut ops = tx.operations.to_vec();
    ops.push(op);
    tx.operations = ops.try_into().map_err(|_| Error::TooManyOperations)?;
    Ok(tx.into())
}
