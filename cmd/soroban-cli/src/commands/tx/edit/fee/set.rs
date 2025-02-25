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


#[derive(clap::Parser, Debug, Clone)]
pub struct Cmd { 
    pub fee: u32
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("only V1 transactions are supported")]
    Unsupported,
    #[error(transparent)]
    XdrStdin(#[from] XdrParsingError),
    #[error(transparent)]
    Xdr(#[from] xdr::Error),
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
        global: &global::Args,
    ) -> Result<(), Error> {
        match tx_env {
            TransactionEnvelope::Tx(transaction_v1_envelope) => {
                    transaction_v1_envelope.tx.fee =
                    self.fee.clone();
            }
            TransactionEnvelope::TxV0(_) | TransactionEnvelope::TxFeeBump(_) => {
                return Err(Error::Unsupported);
            }
        };
        Ok(())
    }
}
