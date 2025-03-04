use crate::{
    commands:: {
        global,
        tx::{edit::precondition::{self, update_max}, xdr::{tx_envelope_from_input, Error as XdrParsingError}},
    },
    xdr::{
        self, TimeBounds, TransactionEnvelope, TransactionV1Envelope, VecM, WriteXdr
    }
};


#[derive(clap::Parser, Debug, Clone)]
pub struct Cmd { 
    max_time_bound: u64
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    XdrStdin(#[from] XdrParsingError),
    #[error(transparent)]
    Xdr(#[from] xdr::Error),
    #[error("only V1 transactions are supported")]
    Unsupported,
    #[error(transparent)]
    Util(#[from] precondition::Error),
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
                update_preconditions(existing_preconditions.clone(), transaction_v1_envelope, self.max_time_bound)?
            }
            TransactionEnvelope::TxV0(_) | TransactionEnvelope::TxFeeBump(_) => {
                return Err(Error::Unsupported);
            }
        };

        Ok(())
    }
}

pub fn update_preconditions(preconditions: xdr::Preconditions, tx_env: &mut TransactionV1Envelope, max_time_bound: u64) -> Result<(), Error> {
    let time_bounds = match preconditions {
        xdr::Preconditions::None => {
            Some(TimeBounds {
                min_time: 0.into(), 
                max_time: max_time_bound.into(),
            })
        }
        xdr::Preconditions::V2(preconditions_v2) => {
            if let Some(time_bounds) = preconditions_v2.time_bounds {
                Some(TimeBounds {
                    min_time: time_bounds.min_time,
                    max_time: max_time_bound.into(),
                })
            } else {
                Some(TimeBounds {
                    min_time: 0.into(), //TODO: is this a sensible default
                    max_time: max_time_bound.into(),
                })
            }
        },
        xdr::Preconditions::Time(time_bounds) => {
            Some(TimeBounds {
                min_time: time_bounds.min_time,
                max_time: max_time_bound.into(),
            })
            // todo() this probably won't happen... we should expect that the preconditions are always either None or V2, with time bounds included in V2
        },
    };
    
    Ok(tx_env.tx.cond = xdr::Preconditions::V2(xdr::PreconditionsV2 {
        time_bounds,
        ledger_bounds: None,
        min_seq_num: None,
        min_seq_age: 0.into(), //FIX ME
        min_seq_ledger_gap: u32::default(), //FIX ME
        extra_signers: VecM::default(),
    }))
}