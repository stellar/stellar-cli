use crate::{
    commands:: {
        global,
        tx::xdr::{tx_envelope_from_stdin, Error as XdrParsingError},
    },
    xdr::{
        self,
        TransactionEnvelope,
        WriteXdr,
    }
};

#[derive(Debug, clap::Parser)]
pub struct Cmd {}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    XdrStdin(#[from] XdrParsingError),
    #[error(transparent)]
    Xdr(#[from] xdr::Error),
    #[error("only V1 transactions are supported")]
    Unsupported,
}

impl Cmd {
    pub fn run(&self, global_args: &global::Args) -> Result<(), Error> { 
        let mut tx = tx_envelope_from_stdin()?;
        self.update_tx_env(&mut tx, global_args)?;
        println!("{}", tx.to_xdr_base64(xdr::Limits::none())?);
        Ok(())
    }

    pub fn update_tx_env(
        &self,
        tx_env: &mut TransactionEnvelope,
        _global: &global::Args,
    ) -> Result<(), Error> {
        match tx_env {
            TransactionEnvelope::Tx(transaction_v1_envelope) => {
                transaction_v1_envelope.tx.memo = xdr::Memo::None;
            }
            TransactionEnvelope::TxV0(_) | TransactionEnvelope::TxFeeBump(_) => {
                return Err(Error::Unsupported);
            }
        };
        Ok(())
    }
}