use clap::Parser;
use hex::FromHexError;
use soroban_env_host::{
    events::HostEvent,
    xdr::{
        AccountId, Error as XdrError, HostFunction, PublicKey, ReadXdr, ScHostStorageErrorCode,
        ScObject, ScSpecEntry, ScStatus, ScVal, Uint256,
    },
    Host, HostError,
};

use crate::{strval::StrValError, utils};

#[derive(Parser, Debug)]
pub struct Cmd {
    /// Contract IDs to filter events on
    #[clap(long = "ids")]
    contract_ids: Vec<String>,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("parsing argument {arg}: {error}")]
    CannotParseArg { arg: String, error: StrValError },
    #[error("cannot parse contract ID {contract_id}: {error}")]
    CannotParseContractId {
        contract_id: String,
        error: FromHexError,
    },
}

#[derive(Clone, Debug)]
enum Arg {
    Arg(String),
    ArgXdr(String),
}

impl Cmd {
    pub async fn run(&self, matches: &clap::ArgMatches) -> Result<(), Error> {
        for raw_contract_id in self.contract_ids.iter() {
            let contract_id: [u8; 32] =
                utils::contract_id_from_str(&raw_contract_id).map_err(|e| {
                    Error::CannotParseContractId {
                        contract_id: raw_contract_id.clone(),
                        error: e,
                    }
                })?;
        }

        Ok(())
    }
}
