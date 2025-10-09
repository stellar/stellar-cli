use std::array::TryFromSliceError;
use std::fmt::Debug;

use super::args::Args;
use crate::{
    commands::config::{self, locator},
    xdr::{self, LedgerKey, LedgerKeyData, MuxedAccount, String64},
};
use clap::{command, Parser};

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub args: Args,

    /// Account alias or public key to lookup, default is test identity
    #[arg(long)]
    pub account: String,

    /// Fetch key-value data entries attached to an account (see manageDataOp)
    #[arg(long, required=true)]
    pub data_name: Vec<String>,

    /// If identity is a seed phrase use this hd path, default is 0
    #[arg(long)]
    pub hd_path: Option<usize>,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Config(#[from] config::key::Error),
    #[error("provided asset is invalid: {0}")]
    InvalidAsset(String),
    #[error("provided data name is invalid: {0}")]
    InvalidDataName(String),
    #[error(transparent)]
    Locator(#[from] locator::Error),
    #[error(transparent)]
    TryFromSliceError(#[from] TryFromSliceError),
    #[error(transparent)]
    Run(#[from] super::args::Error),
}

impl Cmd {
    pub async fn run(&self) -> Result<(), Error> {
        let mut ledger_keys = vec![];
        self.insert_data_keys(&mut ledger_keys)?;
        Ok(self.args.run(ledger_keys).await?)
    }

    fn insert_data_keys(&self, ledger_keys: &mut Vec<LedgerKey>) -> Result<(), Error> {
        let acc = self.muxed_account(&self.account)?;
        for data_name in &self.data_name {
            let data_name: xdr::StringM<64> = data_name
                .parse()
                .map_err(|_| Error::InvalidDataName(data_name.clone()))?;
            let data_name = String64(data_name);
            let key = LedgerKey::Data(LedgerKeyData {
                account_id: acc.clone().account_id(),
                data_name,
            });
            ledger_keys.push(key);
        }

        Ok(())
    }

    fn muxed_account(&self, account: &str) -> Result<MuxedAccount, Error> {
        Ok(self
            .args
            .locator
            .read_key(account)?
            .muxed_account(self.hd_path)?)
    }
}
