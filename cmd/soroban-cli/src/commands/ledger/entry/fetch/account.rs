use std::array::TryFromSliceError;
use std::fmt::Debug;

use super::args::Args;
use crate::{
    commands::config::{self, locator},
    xdr::{
        self, LedgerKey, LedgerKeyAccount, LedgerKeyData, LedgerKeyOffer, MuxedAccount, String64, 
    },
};
use clap::{command, Parser};

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    /// Account alias or public key to lookup, default is test identity
    pub account: String,

    #[command(flatten)]
    pub args: Args,

    /// Fetch key-value data entries attached to an account (see manageDataOp)
    #[arg(long)]
    pub data_name: Option<Vec<String>>,

    /// ID of an offer made on the Stellar DEX
    #[arg(long)]
    pub offer: Option<Vec<i64>>,

    /// Hide the account ledger entry from the output
    #[arg(long)]
    pub hide_account: bool,

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
        self.insert_account_keys(&mut ledger_keys)?;
        self.insert_data_keys(&mut ledger_keys)?;
        self.insert_offer_keys(&mut ledger_keys)?;

        Ok(self.args.run(ledger_keys).await?)
    }

    fn insert_account_keys(&self, ledger_keys: &mut Vec<LedgerKey>) -> Result<(), Error> {
        if self.hide_account {
            return Ok(());
        }
        let acc = self.muxed_account(&self.account)?;
        let key = LedgerKey::Account(LedgerKeyAccount {
            account_id: acc.account_id(),
        });

        ledger_keys.push(key);

        Ok(())
    }

    fn insert_data_keys(&self, ledger_keys: &mut Vec<LedgerKey>) -> Result<(), Error> {
        if let Some(data_name) = &self.data_name {
            let acc = self.muxed_account(&self.account)?;
            for data_name in data_name {
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
        }

        Ok(())
    }

    fn insert_offer_keys(&self, ledger_keys: &mut Vec<LedgerKey>) -> Result<(), Error> {
        if let Some(offer) = &self.offer {
            let acc = self.muxed_account(&self.account)?;
            for offer in offer {
                let key = LedgerKey::Offer(LedgerKeyOffer {
                    seller_id: acc.clone().account_id(),
                    offer_id: *offer,
                });
                ledger_keys.push(key);
            }
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
