use std::{array::TryFromSliceError, fmt::Debug, rc::Rc};

use clap::Parser;
use regex::Regex;
use soroban_env_host::{
    budget::Budget,
    storage::Storage,
    xdr::{
        AccountId, AlphaNum12, AlphaNum4, Asset, AssetCode12, AssetCode4, Error as XdrError,
        HostFunction, PublicKey, ScHostStorageErrorCode, ScObject, ScStatus, ScVal, WriteXdr,
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
    #[error("cannot parse asset: {asset}")]
    CannotParseAsset { asset: String },
    #[error("invalid asset code: {asset}")]
    InvalidAssetCode { asset: String },
    #[error("cannot parse account id: {account_id}")]
    CannotParseAccountId { account_id: String },
    #[error("internal conversion error: {0}")]
    TryFromSliceError(#[from] TryFromSliceError),
    #[error("xdr processing error: {0}")]
    Xdr(#[from] XdrError),
}

#[derive(Parser, Debug)]
pub struct Cmd {
    /// ID of the Stellar classic asset to wrap, e.g. "USDC:G...5"
    #[clap(long)]
    asset: String,

    /// File to persist ledger state
    #[clap(long, parse(from_os_str), default_value(".soroban/ledger.json"))]
    ledger_file: std::path::PathBuf,
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        let res_str = self.run_in_sandbox(&self.asset)?;
        println!("{}", res_str);
        Ok(())
    }

    fn run_in_sandbox(&self, asset_str: &str) -> Result<String, Error> {
        // Parse asset
        let asset = Cmd::parse_asset(asset_str)?;

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

        let mut ledger_info = state.0.clone();
        ledger_info.sequence_number += 1;
        ledger_info.timestamp += 5;
        h.set_ledger_info(ledger_info.clone());

        let mut buf: Vec<u8> = vec![];
        asset.write_xdr(&mut buf)?;
        let final_args = vec![ScVal::Object(Some(ScObject::Bytes(buf.try_into()?)))]
            .try_into()
            .expect("invalid arguments");

        let res = h.invoke_function(HostFunction::CreateTokenContractWithAsset, final_args)?;
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

    fn parse_asset(str: &str) -> Result<Asset, Error> {
        if str == "native" {
            return Ok(Asset::Native);
        }
        let split: Vec<&str> = str.splitn(2, ':').collect();
        if split.len() != 2 {
            return Err(Error::CannotParseAsset {
                asset: str.to_string(),
            });
        }
        let code = split[0];
        let issuer = split[1];
        let re = Regex::new("^[[:alnum:]]{1,12}$").unwrap();
        if !re.is_match(code) {
            return Err(Error::InvalidAssetCode {
                asset: str.to_string(),
            });
        }
        if code.len() <= 4 {
            let mut asset_code: [u8; 4] = [0; 4];
            for (i, b) in code.as_bytes().iter().enumerate() {
                asset_code[i] = *b;
            }
            Ok(Asset::CreditAlphanum4(AlphaNum4 {
                asset_code: AssetCode4(asset_code),
                issuer: Cmd::parse_account_id(issuer)?,
            }))
        } else {
            let mut asset_code: [u8; 12] = [0; 12];
            for (i, b) in code.as_bytes().iter().enumerate() {
                asset_code[i] = *b;
            }
            Ok(Asset::CreditAlphanum12(AlphaNum12 {
                asset_code: AssetCode12(asset_code),
                issuer: Cmd::parse_account_id(issuer)?,
            }))
        }
    }

    fn parse_account_id(str: &str) -> Result<AccountId, Error> {
        let pk_bytes = StrkeyPublicKeyEd25519::from_string(str)
            .map_err(|_| Error::CannotParseAccountId {
                account_id: str.to_string(),
            })?
            .0;
        Ok(AccountId(PublicKey::PublicKeyTypeEd25519(pk_bytes.into())))
    }
}
