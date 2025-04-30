use crate::xdr::{
    Limits, Operation, ReadXdr, Transaction, TransactionEnvelope, TransactionV1Envelope,
};
use std::ffi::OsString;
use std::fs::File;
use std::io::Cursor;
use std::io::{stdin, Read};
use std::path::Path;
use stellar_xdr::curr::Limited;

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
        &mut stdin()
    };

    let mut lim = Limited::new(SkipWhitespace::new(read), Limits::none());
    Ok(TransactionEnvelope::read_xdr_base64_to_end(&mut lim)?)
}

// TODO: use SkipWhitespace from rs-stellar-xdr once it's updated to 23.0
pub struct SkipWhitespace<R: Read> {
    pub inner: R,
}

impl<R: Read> SkipWhitespace<R> {
    pub fn new(inner: R) -> Self {
        SkipWhitespace { inner }
    }
}

impl<R: Read> Read for SkipWhitespace<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.inner.read(buf)?;

        let mut written = 0;
        for read in 0..n {
            if !buf[read].is_ascii_whitespace() {
                buf[written] = buf[read];
                written += 1;
            }
        }

        Ok(written)
    }
}
//

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
