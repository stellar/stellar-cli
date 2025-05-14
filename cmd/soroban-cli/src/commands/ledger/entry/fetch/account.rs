use std::array::TryFromSliceError;
use std::fmt::Debug;

use crate::commands::config::network;
use crate::config::locator;
use crate::rpc;
use crate::{config, xdr};
use clap::{command, Parser};
use stellar_strkey::ed25519::PublicKey as Ed25519PublicKey;
use stellar_xdr::curr::{
    AccountId, AlphaNum12, AlphaNum4, AssetCode12, AssetCode4, LedgerKey, LedgerKeyAccount,
    LedgerKeyData, LedgerKeyOffer, LedgerKeyTrustLine, MuxedAccount, PublicKey, String64,
    TrustLineAsset, Uint256,
};

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    /// Account alias or public key to lookup, default is test identity
    pub account: String,

    #[command(flatten)]
    pub network: network::Args,

    #[command(flatten)]
    pub locator: locator::Args,

    //Options
    /// Assets to get trustline info for
    #[arg(long)]
    pub asset: Option<Vec<String>>,

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

    /// Format of the output
    #[arg(long, default_value = "json")]
    pub output: OutputFormat,
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
    Network(#[from] network::Error),
    #[error(transparent)]
    Rpc(#[from] rpc::Error),
    #[error(transparent)]
    Serde(#[from] serde_json::Error),
    #[error(transparent)]
    TryFromSliceError(#[from] TryFromSliceError),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, clap::ValueEnum, Default)]
pub enum OutputFormat {
    /// JSON output of the ledger entry with parsed XDRs (one line, not formatted)
    #[default]
    Json,
    /// Formatted (multiline) JSON output of the ledger entry with parsed XDRs
    JsonFormatted,
    /// Original RPC output (containing XDRs)
    Xdr,
}

impl Cmd {
    pub async fn run(&self) -> Result<(), Error> {
        let mut ledger_keys = vec![];

        self.insert_account_keys(&mut ledger_keys)?;
        self.insert_asset_keys(&mut ledger_keys)?;
        self.insert_data_keys(&mut ledger_keys)?;
        self.insert_offer_keys(&mut ledger_keys)?;

        let network = self.network.get(&self.locator)?;
        let client = network.rpc_client()?;
        match self.output {
            OutputFormat::Json => {
                let resp = client.get_full_ledger_entries(&ledger_keys).await?;
                println!("{}", serde_json::to_string(&resp)?);
            }
            OutputFormat::Xdr => {
                let resp = client.get_ledger_entries(&ledger_keys).await?;
                println!("{}", serde_json::to_string(&resp)?);
            }
            OutputFormat::JsonFormatted => {
                let resp = client.get_full_ledger_entries(&ledger_keys).await?;
                println!("{}", serde_json::to_string_pretty(&resp)?);
            }
        }

        Ok(())
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
            .locator
            .read_key(account)?
            .muxed_account(self.hd_path)?)
    }
}
