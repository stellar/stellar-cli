use std::array::TryFromSliceError;
use std::fmt::Debug;

use super::args::Args;
use crate::{
    commands::config::{self, locator},
    xdr::{LedgerKey, LedgerKeyOffer, MuxedAccount},
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

    /// ID of an offer made on the Stellar DEX
    #[arg(long, required=true)]
    pub offer: Vec<i64>,

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
        self.insert_offer_keys(&mut ledger_keys)?;

        Ok(self.args.run(ledger_keys).await?)
    }

    fn insert_offer_keys(&self, ledger_keys: &mut Vec<LedgerKey>) -> Result<(), Error> {
        let acc = self.muxed_account(&self.account)?;
        for offer in &self.offer {
            let key = LedgerKey::Offer(LedgerKeyOffer {
                seller_id: acc.clone().account_id(),
                offer_id: *offer,
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
