use soroban_env_host::xdr::{self, DiagnosticEvent, ScVal};

use super::Error;
use crate::rpc::{extract_events, GetTransactionResponse, SimulateTransactionResponse};

pub enum Kind {
    Simulated(SimulateTransactionResponse),
    Signed(GetTransactionResponse),
}

pub struct Finished {
    txn_res: Kind,
}

impl Finished {
    pub fn simulated(txn_res: SimulateTransactionResponse) -> Self {
        Self {
            txn_res: Kind::Simulated(txn_res),
        }
    }

    pub fn signed(txn_res: GetTransactionResponse) -> Self {
        Self {
            txn_res: Kind::Signed(txn_res),
        }
    }

    pub fn return_value(&self) -> Result<ScVal, Error> {
        match &self.txn_res {
            Kind::Simulated(sim_res) => Ok(sim_res
                .results()?
                .get(0)
                .ok_or(Error::MissingOp)?
                .xdr
                .clone()),
            Kind::Signed(GetTransactionResponse {
                result_meta:
                    Some(xdr::TransactionMeta::V3(xdr::TransactionMetaV3 {
                        soroban_meta: Some(xdr::SorobanTransactionMeta { return_value, .. }),
                        ..
                    })),
                ..
            }) => Ok(return_value.clone()),
            Kind::Signed(_) => Err(Error::MissingOp),
        }
    }

    pub fn events(&self) -> Result<Vec<DiagnosticEvent>, Error> {
        match &self.txn_res {
            Kind::Simulated(sim_res) => sim_res.events(),
            Kind::Signed(GetTransactionResponse {
                result_meta: Some(meta),
                ..
            }) => Ok(extract_events(meta)),
            Kind::Signed(_) => Err(Error::MissingOp),
        }
    }

    pub fn as_signed(&self) -> Result<&GetTransactionResponse, Error> {
        if let Kind::Signed(res) = &self.txn_res {
            Ok(res)
        } else {
            Err(Error::NotSignedTransaction)
        }
    }
}
