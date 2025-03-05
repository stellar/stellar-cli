use crate::xdr::{self, TimeBounds, TransactionV1Envelope, VecM};

#[derive(Default)]
pub struct Args {
    pub max_time_bound: Option<u64>,
    pub min_time_bound: Option<u64>,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {}

impl Args {
    pub fn update_preconditions(
        &self,
        preconditions: xdr::Preconditions,
        tx_env: &mut TransactionV1Envelope,
    ) -> Result<(), Error> {
        if self.max_time_bound.is_some() {
            update_max(preconditions, tx_env, self.max_time_bound.unwrap())
        } else if self.min_time_bound.is_some() {
            update_min(preconditions, tx_env, self.min_time_bound.unwrap())
        } else {
            Ok(())
        }
    }
}

pub fn update_min(
    preconditions: xdr::Preconditions,
    tx_env: &mut TransactionV1Envelope,
    min_time_bound: u64,
) -> Result<(), Error> {
    let time_bounds = match preconditions {
        xdr::Preconditions::None => Some(TimeBounds {
            min_time: min_time_bound.into(),
            max_time: 0.into(),
        }),
        xdr::Preconditions::V2(preconditions_v2) => {
            if let Some(time_bounds) = preconditions_v2.time_bounds {
                Some(TimeBounds {
                    min_time: min_time_bound.into(),
                    max_time: time_bounds.max_time,
                })
            } else {
                Some(TimeBounds {
                    min_time: min_time_bound.into(),
                    max_time: u64::MAX.into(),
                })
            }
        }
        xdr::Preconditions::Time(time_bounds) => {
            Some(TimeBounds {
                min_time: min_time_bound.into(),
                max_time: time_bounds.max_time,
            })
            // todo() this probably won't happen... we should expect that the preconditions are always either None or V2, with time bounds included in V2
        }
    };

    Ok(
        tx_env.tx.cond = xdr::Preconditions::V2(xdr::PreconditionsV2 {
            time_bounds,
            ledger_bounds: None,
            min_seq_num: None,
            min_seq_age: 0.into(),              //FIX ME
            min_seq_ledger_gap: u32::default(), //FIX ME
            extra_signers: VecM::default(),
        }),
    )
}

pub fn update_max(
    preconditions: xdr::Preconditions,
    tx_env: &mut TransactionV1Envelope,
    max_time_bound: u64,
) -> Result<(), Error> {
    let time_bounds = match preconditions {
        xdr::Preconditions::None => Some(TimeBounds {
            min_time: 0.into(),
            max_time: max_time_bound.into(),
        }),
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
        }
        xdr::Preconditions::Time(time_bounds) => {
            Some(TimeBounds {
                min_time: time_bounds.min_time,
                max_time: max_time_bound.into(),
            })
            // todo() this probably won't happen... we should expect that the preconditions are always either None or V2, with time bounds included in V2
        }
    };

    Ok(
        tx_env.tx.cond = xdr::Preconditions::V2(xdr::PreconditionsV2 {
            time_bounds,
            ledger_bounds: None,
            min_seq_num: None,
            min_seq_age: 0.into(),              //FIX ME
            min_seq_ledger_gap: u32::default(), //FIX ME
            extra_signers: VecM::default(),
        }),
    )
}
