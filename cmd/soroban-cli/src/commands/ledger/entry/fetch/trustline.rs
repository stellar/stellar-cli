use std::array::TryFromSliceError;
use std::fmt::Debug;

use super::args::Args;
use crate::{
    commands::config::{self, locator},
    xdr::{ AccountId, AlphaNum12, AlphaNum4, AssetCode12, AssetCode4, LedgerKey, LedgerKeyTrustLine, MuxedAccount, PublicKey, TrustLineAsset, Uint256,
    },
};
use clap::{command, Parser};
use stellar_strkey::ed25519::PublicKey as Ed25519PublicKey;

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub args: Args,

    /// Account alias or public key to lookup, default is test identity
    #[arg(long)]
    pub account: String,

    /// Assets to get trustline info for
    #[arg(long)]
    pub asset: Option<Vec<String>>,

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
        self.insert_asset_keys(&mut ledger_keys)?;
        Ok(self.args.run(ledger_keys).await?)
    }

    fn insert_asset_keys(&self, ledger_keys: &mut Vec<LedgerKey>) -> Result<(), Error> {
        if let Some(asset) = &self.asset {
            let acc = self.muxed_account(&self.account)?;
            for asset in asset {
                let asset = if asset.eq_ignore_ascii_case("XLM") {
                    TrustLineAsset::Native
                } else if asset.contains(':') {
                    let mut parts = asset.split(':');
                    let code = parts.next().ok_or(Error::InvalidAsset(asset.clone()))?;
                    let issuer = parts.next().ok_or(Error::InvalidAsset(asset.clone()))?;
                    if parts.next().is_some() {
                        Err(Error::InvalidAsset(asset.clone()))?;
                    }
                    let source_bytes = Ed25519PublicKey::from_string(issuer).unwrap().0;
                    let issuer = AccountId(PublicKey::PublicKeyTypeEd25519(Uint256(source_bytes)));

                    match code.len() {
                        4 => TrustLineAsset::CreditAlphanum4(AlphaNum4 {
                            asset_code: AssetCode4(code.as_bytes().try_into()?),
                            issuer,
                        }),
                        12 => TrustLineAsset::CreditAlphanum12(AlphaNum12 {
                            asset_code: AssetCode12(code.as_bytes().try_into()?),
                            issuer,
                        }),
                        _ => Err(Error::InvalidAsset(asset.clone()))?,
                    }
                } else {
                    Err(Error::InvalidAsset(asset.clone()))?
                };

                let key = LedgerKey::Trustline(LedgerKeyTrustLine {
                    account_id: acc.clone().account_id(),
                    asset,
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
