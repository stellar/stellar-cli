use crate::{
    commands::{
        global,
        tx::{
            edit::precondition,
            xdr::{tx_envelope_from_input, Error as XdrParsingError},
        },
    },
    xdr::{
        self, TransactionEnvelope, WriteXdr,
    },
};

#[derive(clap::Parser, Debug, Clone)]
pub struct Cmd {
    min_time_bound: u64,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Util(#[from] precondition::Error),
    #[error(transparent)]
    XdrStdin(#[from] XdrParsingError),
    #[error(transparent)]
    Xdr(#[from] xdr::Error),
    #[error("only V1 transactions are supported")]
    Unsupported,
}

impl Cmd {
    pub fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let mut tx = tx_envelope_from_input(&None)?;
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
                let existing_preconditions = &transaction_v1_envelope.tx.cond;
                let args = precondition::Args {
                    min_time_bound: Some(self.min_time_bound),
                    ..Default::default()
                };
                args.update_preconditions(existing_preconditions.clone(), transaction_v1_envelope)?
            }
            TransactionEnvelope::TxV0(_) | TransactionEnvelope::TxFeeBump(_) => {
                return Err(Error::Unsupported);
            }
        };

        Ok(())
    }
}
