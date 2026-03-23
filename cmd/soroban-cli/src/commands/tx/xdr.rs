use crate::xdr::{
    Limits, Operation, ReadXdr, Transaction, TransactionEnvelope, TransactionV1Envelope,
};
use std::ffi::OsString;
use std::fs::File;
use std::io::{stdin, Read};
use std::io::{Cursor, IsTerminal};
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
        loop {
            let n = self.inner.read(buf)?;
            if n == 0 {
                return Ok(0);
            }

            let mut written = 0;
            for read in 0..n {
                if !buf[read].is_ascii_whitespace() {
                    buf[written] = buf[read];
                    written += 1;
                }
            }

            if written > 0 {
                return Ok(written);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn skip_whitespace_preserves_content() {
        let input = Cursor::new(b"helloworld");
        let mut reader = SkipWhitespace::new(input);
        let mut result = String::new();
        reader.read_to_string(&mut result).unwrap();
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn skip_whitespace_strips_all_whitespace_types() {
        let input = Cursor::new(b"hello \t\n\r world");
        let mut reader = SkipWhitespace::new(input);
        let mut result = String::new();
        reader.read_to_string(&mut result).unwrap();
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn skip_whitespace_handles_only_whitespace() {
        let input = Cursor::new(b"\n \t \r\n");
        let mut reader = SkipWhitespace::new(input);
        let mut result = String::new();
        reader.read_to_string(&mut result).unwrap();
        assert_eq!(result, "");
    }

    #[test]
    fn skip_whitespace_handles_empty_input() {
        let input = Cursor::new(b"");
        let mut reader = SkipWhitespace::new(input);
        let mut result = String::new();
        reader.read_to_string(&mut result).unwrap();
        assert_eq!(result, "");
    }

    #[test]
    fn skip_whitespace_handles_leading_trailing_whitespace() {
        let input = Cursor::new(b"\n\nhello\n\n");
        let mut reader = SkipWhitespace::new(input);
        let mut result = String::new();
        reader.read_to_string(&mut result).unwrap();
        assert_eq!(result, "hello");
    }
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
