use std::array::TryFromSliceError;
use std::fmt::Debug;

use crate::commands::config::network;
use crate::commands::contract::Durability;
use crate::commands::ledger::entry::fetch::Error::{
    AccountRequired, ContractRequired, EmptyKeys, InvalidAsset, InvalidConfigId, InvalidDataName,
    InvalidHash,
};
use crate::config::locator;
use crate::rpc::{self};
use crate::{config, xdr};
use clap::{command, Parser};
use hex::FromHexError;
use soroban_spec_tools::utils::padded_hex_from_str;
use stellar_strkey::ed25519::PublicKey as Ed25519PublicKey;
use stellar_xdr::curr::{
    ClaimableBalanceId::ClaimableBalanceIdTypeV0,
    AccountId, AlphaNum12, AlphaNum4, AssetCode12, AssetCode4, ConfigSettingId,
    ContractDataDurability, Hash, LedgerKey, LedgerKeyAccount, LedgerKeyClaimableBalance,
    LedgerKeyConfigSetting, LedgerKeyContractCode, LedgerKeyContractData, LedgerKeyData,
    LedgerKeyLiquidityPool, LedgerKeyOffer, LedgerKeyTrustLine, LedgerKeyTtl, Limits, MuxedAccount,
    PoolId, PublicKey, ReadXdr, ScAddress, ScVal, String64, TrustLineAsset, Uint256,
};
use crate::config::network::Network;

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub network: network::Args,
    #[command(flatten)]
    pub locator: locator::Args,

    /// Name of identity to lookup, default is test identity
    #[arg(long)]
    pub account: Option<String>,
    /// If identity is a seed phrase use this hd path, default is 0
    #[arg(long)]
    pub hd_path: Option<usize>,

    /// Assets to get trustline info for
    #[arg(long)]
    pub asset: Option<Vec<String>>,
    /// ID of an offer made on the Stellar DEX
    #[arg(long)]
    pub offer: Option<Vec<i64>>,
    /// Fetch key-value data entries attached to an account (see manageDataOp)
    #[arg(long)]
    pub data_name: Option<Vec<String>>,

    /// Claimable Balance id
    #[arg(long)]
    pub claimable_id: Option<Vec<String>>,

    /// Liquidity pool id
    #[arg(long)]
    pub pool_id: Option<Vec<String>>,

    /// Defines the currently active network configuration
    #[arg(long)]
    pub config_setting_id: Option<Vec<i32>>,

    /// Get WASM bytecode by hash
    #[arg(long)]
    pub wasm_hash: Option<Vec<String>>,

    /// Get the time-to-live of an associated contract data or code entry
    #[arg(long)]
    pub ttl: Option<Vec<String>>,

    /// Contract id to fetch an info for
    #[arg(long = "id", env = "STELLAR_CONTRACT_ID")]
    pub contract_id: Option<config::UnresolvedContract>,
    /// Storage entry durability
    #[arg(long, value_enum, default_value = "persistent")]
    pub durability: Durability,
    /// Storage key (symbols only)
    #[arg(long = "key")]
    pub key: Option<Vec<String>>,
    /// Storage key (base64-encoded XDR)
    #[arg(long = "key-xdr")]
    pub key_xdr: Option<Vec<String>>,

    /// Format of the output
    #[arg(long, default_value = "json")]
    pub output: OutputFormat,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Config(#[from] config::key::Error),
    #[error(transparent)]
    Locator(#[from] locator::Error),
    #[error(transparent)]
    Rpc(#[from] rpc::Error),
    #[error(transparent)]
    Network(#[from] network::Error),
    #[error(transparent)]
    Serde(#[from] serde_json::Error),
    #[error(transparent)]
    StellarXdr(#[from] stellar_xdr::curr::Error),
    #[error(transparent)]
    Spec(#[from] soroban_spec_tools::Error),
    #[error(transparent)]
    TryFromSliceError(#[from] TryFromSliceError),
    #[error(transparent)]
    FromHexError(#[from] FromHexError),
    #[error("at least one key must be provided")]
    EmptyKeys,
    #[error("contract id is required but was not provided")]
    ContractRequired,
    #[error("account is required but was not provided")]
    AccountRequired,
    #[error("provided asset is invalid: {0}")]
    InvalidAsset(String),
    #[error("provided data name is invalid: {0}")]
    InvalidDataName(String),
    #[error("provided hash value is invalid: {0}")]
    InvalidHash(String),
    #[error("provided config id is invalid: {0}")]
    InvalidConfigId(i32),
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
        let network = self.network.get(&self.locator)?;
        let client = network.rpc_client()?;
        let mut ledger_keys = vec![];

        self.insert_contract_keys(&network, &mut ledger_keys)?;

        self.insert_account_keys(&mut ledger_keys)?;

        if let Some(claimable_id) = &self.claimable_id {
            for x in claimable_id {
                let hash = Hash(padded_hex_from_str(x, 32)?.try_into().unwrap());
                let key = LedgerKey::ClaimableBalance(LedgerKeyClaimableBalance {
                    balance_id: ClaimableBalanceIdTypeV0(hash),
                });
                ledger_keys.push(key);
            }
        }

        if let Some(pool_id) = &self.pool_id {
            for x in pool_id {
                let hash = Hash(padded_hex_from_str(x, 32)?.try_into().unwrap());
                let key = LedgerKey::LiquidityPool(LedgerKeyLiquidityPool {
                    liquidity_pool_id: PoolId(hash),
                });
                ledger_keys.push(key);
            }
        }

        if let Some(wasm_hash) = &self.wasm_hash {
            for wasm_hash in wasm_hash {
                let hash = Hash(
                    soroban_spec_tools::utils::contract_id_from_str(wasm_hash)
                        .map_err(|_| InvalidHash(wasm_hash.clone()))?,
                );
                let key = LedgerKey::ContractCode(LedgerKeyContractCode { hash });
                ledger_keys.push(key);
            }
        }

        if let Some(config_setting_id) = &self.config_setting_id {
            for x in config_setting_id {
                let key = LedgerKey::ConfigSetting(LedgerKeyConfigSetting {
                    config_setting_id: ConfigSettingId::try_from(*x)
                        .map_err(|_| InvalidConfigId(*x))?,
                });
                ledger_keys.push(key);
            }
        }

        if let Some(ttl) = &self.ttl {
            for x in ttl {
                let hash = Hash(padded_hex_from_str(x, 32)?.try_into().unwrap());
                let key = LedgerKey::Ttl(LedgerKeyTtl { key_hash: hash });
                ledger_keys.push(key);
            }
        }

        if ledger_keys.is_empty() {
            return Err(EmptyKeys);
        }

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
        if let Some(acc) = &self.account {
            let acc = self.muxed_account(acc)?;

            if let Some(asset) = &self.asset {
                for asset in asset {
                    let asset = if asset.eq_ignore_ascii_case("XLM") {
                        TrustLineAsset::Native
                    } else if asset.contains(':') {
                        let mut parts = asset.split(':');
                        let code = parts.next().ok_or(InvalidAsset(asset.clone()))?;
                        let issuer = parts.next().ok_or(InvalidAsset(asset.clone()))?;
                        if parts.next().is_some() {
                            Err(InvalidAsset(asset.clone()))?;
                        }
                        let source_bytes = Ed25519PublicKey::from_string(issuer).unwrap().0;
                        let issuer =
                            AccountId(PublicKey::PublicKeyTypeEd25519(Uint256(source_bytes)));

                        match code.len() {
                            4 => TrustLineAsset::CreditAlphanum4(AlphaNum4 {
                                asset_code: AssetCode4(code.as_bytes().try_into()?),
                                issuer,
                            }),
                            12 => TrustLineAsset::CreditAlphanum12(AlphaNum12 {
                                asset_code: AssetCode12(code.as_bytes().try_into()?),
                                issuer,
                            }),
                            _ => Err(InvalidAsset(asset.clone()))?,
                        }
                    } else {
                        Err(InvalidAsset(asset.clone()))?
                    };

                    let key = LedgerKey::Trustline(LedgerKeyTrustLine {
                        account_id: acc.clone().account_id(),
                        asset,
                    });

                    ledger_keys.push(key);
                }
            }

            if let Some(offer) = &self.offer {
                for offer in offer {
                    let key = LedgerKey::Offer(LedgerKeyOffer {
                        seller_id: acc.clone().account_id(),
                        offer_id: *offer,
                    });
                    ledger_keys.push(key);
                }
            }

            if let Some(data_name) = &self.data_name {
                for data_name in data_name {
                    let data_name: xdr::StringM<64> = data_name
                        .parse()
                        .map_err(|_| InvalidDataName(data_name.clone()))?;
                    let data_name = String64(data_name);
                    let key = LedgerKey::Data(LedgerKeyData {
                        account_id: acc.clone().account_id(),
                        data_name,
                    });
                    ledger_keys.push(key);
                }
            }

            // always add the account key into the list
            // should we allow this to be configurable?
            let key = LedgerKey::Account(LedgerKeyAccount {
                account_id: acc.account_id(),
            });

            ledger_keys.push(key);
        } else if self.asset.is_some() || self.offer.is_some() || self.data_name.is_some() {
            return Err(AccountRequired);
        }

        Ok(())
    }

    fn insert_contract_keys(&self, network: &Network, ledger_keys: &mut Vec<LedgerKey>) -> Result<(), Error> {
        if let Some(contract_id) = &self.contract_id {
            let contract_id =
                contract_id.resolve_contract_id(&self.locator, &network.network_passphrase)?;

            let contract_address_arg = ScAddress::Contract(Hash(contract_id.0));

            if let Some(keys) = &self.key {
                for key in keys {
                    let key = LedgerKey::ContractData(LedgerKeyContractData {
                        contract: contract_address_arg.clone(),
                        key: soroban_spec_tools::from_string_primitive(
                            key,
                            &xdr::ScSpecTypeDef::Symbol,
                        )?,
                        durability: ContractDataDurability::Persistent,
                    });

                    ledger_keys.push(key);
                }
            }

            if let Some(keys) = &self.key_xdr {
                for key in keys {
                    let key = LedgerKey::ContractData(LedgerKeyContractData {
                        contract: contract_address_arg.clone(),
                        key: ScVal::from_xdr_base64(key, Limits::none())?,
                        durability: ContractDataDurability::Persistent,
                    });

                    ledger_keys.push(key);
                }
            }
        } else if self.key.is_some() || self.key_xdr.is_some() {
            return Err(ContractRequired);
        }

        Ok(())
    }

    fn muxed_account(&self, account: &str) -> Result<MuxedAccount, Error> {
        Ok(self
            .locator
            .read_identity(account)?
            .muxed_account(self.hd_path)?)
    }
}
