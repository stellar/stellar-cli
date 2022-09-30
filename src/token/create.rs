use std::{fmt::Debug, rc::Rc};

use clap::Parser;
use soroban_env_host::{
    budget::Budget,
    storage::Storage,
    xdr::{
        AccountId, Error as XdrError, HostFunction, PublicKey, ScHostStorageErrorCode, ScObject,
        ScStatus, ScVal, Uint256,
    },
    Host, HostError,
};
use stellar_strkey::StrkeyPublicKeyEd25519;

use crate::{snapshot, utils};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    // TODO: the Display impl of host errors is pretty user-unfriendly
    //       (it just calls Debug). I think we can do better than that
    Host(#[from] HostError),
    #[error("reading file {filepath}: {error}")]
    CannotReadLedgerFile {
        filepath: std::path::PathBuf,
        error: snapshot::Error,
    },
    #[error("committing file {filepath}: {error}")]
    CannotCommitLedgerFile {
        filepath: std::path::PathBuf,
        error: snapshot::Error,
    },
    #[error("cannot parse salt: {salt}")]
    CannotParseSalt { salt: String },
    #[error("xdr processing error: {0}")]
    Xdr(#[from] XdrError),
}

#[derive(Parser, Debug)]
pub struct Cmd {
    /// Administrator account for the token
    /// TODO: Do we need this? Or use source of deployer?
    #[clap(
        long,
        default_value = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF"
    )]
    admin: StrkeyPublicKeyEd25519,

    /// Number of decimal places for the token
    #[clap(long, default_value = "7")]
    decimal: u32,

    /// Long name of the token, e.g. "Stellar Lumens"
    #[clap(long)]
    name: String,

    /// Short name of the token, e.g. "XLM"
    #[clap(long)]
    symbol: String,

    /// Custom salt 32-byte salt for the token id
    #[clap(
        long,
        default_value = "0000000000000000000000000000000000000000000000000000000000000000"
    )]
    salt: String,

    /// File to persist ledger state
    #[clap(long, parse(from_os_str), default_value(".soroban/ledger.json"))]
    ledger_file: std::path::PathBuf,
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        // Hack: re-use contract_id_from_str to parse the 32-byte salt hex.
        let salt: [u8; 32] =
            utils::contract_id_from_str(&self.salt).map_err(|_| Error::CannotParseSalt {
                salt: self.salt.clone(),
            })?;

        let res_str = self.run_in_sandbox(salt)?;
        println!("{}", res_str);
        Ok(())
    }

    fn run_in_sandbox(&self, salt: [u8; 32]) -> Result<String, Error> {
        // Initialize storage and host
        // TODO: allow option to separate input and output file
        let state = snapshot::read(&self.ledger_file).map_err(|e| Error::CannotReadLedgerFile {
            filepath: self.ledger_file.clone(),
            error: e,
        })?;

        let snap = Rc::new(snapshot::Snap {
            ledger_entries: state.1.clone(),
        });
        let h = Host::with_storage_and_budget(
            Storage::with_recording_footprint(snap),
            Budget::default(),
        );

        h.set_source_account(AccountId(PublicKey::PublicKeyTypeEd25519(Uint256(
            self.admin.0,
        ))));

        let mut ledger_info = state.0.clone();
        ledger_info.sequence_number += 1;
        ledger_info.timestamp += 5;
        h.set_ledger_info(ledger_info.clone());

        let final_args = vec![ScVal::Object(Some(ScObject::Bytes(salt.try_into()?)))]
            .try_into()
            .expect("invalid arguments");
        let res = h.invoke_function(
            HostFunction::CreateTokenContractWithSourceAccount,
            final_args,
        )?;
        let res_str = utils::vec_to_hash(&res)?;

        let (storage, _, _) = h.try_finish().map_err(|_h| {
            HostError::from(ScStatus::HostStorageError(
                ScHostStorageErrorCode::UnknownError,
            ))
        })?;

        snapshot::commit(state.1, ledger_info, &storage.map, &self.ledger_file).map_err(|e| {
            Error::CannotCommitLedgerFile {
                filepath: self.ledger_file.clone(),
                error: e,
            }
        })?;
        Ok(res_str)
    }
}
