use crate::utils::XDR_DEPTH_LIMIT;
use crate::xdr::{
    Limits, Operation, ReadXdr, Transaction, TransactionEnvelope, TransactionV1Envelope,
};
use std::ffi::OsString;
use std::fs::File;
use std::io::{stdin, Read};
use std::io::{Cursor, IsTerminal};
use std::path::Path;
use stellar_xdr::Limited;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to decode XDR: {0}")]
    XDRDecode(#[from] stellar_xdr::Error),
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

    let mut lim = Limited::new(SkipWhitespace::new(read), Limits::depth(XDR_DEPTH_LIMIT));
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

/// Number of signatures already attached to the envelope, across all envelope
/// flavors. Callers that go on to discard the envelope's signatures (e.g. via
/// [`unwrap_envelope_v1`]) can use this to say so instead of dropping them
/// silently.
pub fn signature_count(tx_env: &TransactionEnvelope) -> usize {
    match tx_env {
        TransactionEnvelope::TxV0(e) => e.signatures.len(),
        TransactionEnvelope::Tx(e) => e.signatures.len(),
        TransactionEnvelope::TxFeeBump(e) => e.signatures.len(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    struct ChunkedReader {
        chunks: Vec<Vec<u8>>,
        pos: usize,
    }

    impl ChunkedReader {
        fn new(chunks: Vec<&[u8]>) -> Self {
            Self {
                chunks: chunks.iter().map(|c| c.to_vec()).collect(),
                pos: 0,
            }
        }
    }

    impl Read for ChunkedReader {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            if self.pos >= self.chunks.len() {
                return Ok(0);
            }
            let chunk = &self.chunks[self.pos];
            let n = chunk.len().min(buf.len());
            buf[..n].copy_from_slice(&chunk[..n]);
            self.pos += 1;
            Ok(n)
        }
    }

    fn minimal_tx() -> Transaction {
        use crate::xdr::{
            Memo, MuxedAccount, Preconditions, SequenceNumber, TransactionExt, Uint256,
        };
        Transaction {
            source_account: MuxedAccount::Ed25519(Uint256([0u8; 32])),
            fee: 100,
            seq_num: SequenceNumber(1),
            cond: Preconditions::None,
            memo: Memo::None,
            operations: Vec::new().try_into().unwrap(),
            ext: TransactionExt::V0,
        }
    }

    fn signatures(n: usize) -> crate::xdr::VecM<crate::xdr::DecoratedSignature, 20> {
        use crate::xdr::{DecoratedSignature, Signature, SignatureHint};
        vec![
            DecoratedSignature {
                hint: SignatureHint([0u8; 4]),
                signature: Signature(vec![0u8; 64].try_into().unwrap()),
            };
            n
        ]
        .try_into()
        .unwrap()
    }

    #[test]
    fn signature_count_reports_attached_signatures() {
        let unsigned = TransactionEnvelope::Tx(TransactionV1Envelope {
            tx: minimal_tx(),
            signatures: signatures(0),
        });
        assert_eq!(signature_count(&unsigned), 0);

        let signed = TransactionEnvelope::Tx(TransactionV1Envelope {
            tx: minimal_tx(),
            signatures: signatures(2),
        });
        assert_eq!(signature_count(&signed), 2);
    }

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
    fn skip_whitespace_loops_past_whitespace_only_chunks() {
        // Exercises the loop iterating more than once: first chunk is all
        // whitespace, second chunk has content. A Cursor would satisfy both
        // reads in one shot and would never trigger the loop.
        let reader = ChunkedReader::new(vec![b"\n\n", b"hello", b""]);
        let mut skipper = SkipWhitespace::new(reader);
        let mut result = String::new();
        skipper.read_to_string(&mut result).unwrap();
        assert_eq!(result, "hello");
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
