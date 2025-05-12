use std::array::TryFromSliceError;
use std::fmt::Debug;

use crate::commands::config::network;
use crate::commands::contract::Durability;
use crate::config::locator;
use crate::config::network::Network;
use crate::rpc::{self};
use crate::{config, xdr};
use clap::{command, Parser};
use hex::{FromHex, FromHexError};
use soroban_spec_tools::utils::padded_hex_from_str;
use stellar_strkey::Strkey;
use stellar_strkey::{ed25519::PublicKey as Ed25519PublicKey, Contract};
use stellar_xdr::curr::{
    AccountId, AlphaNum12, AlphaNum4, AssetCode12, AssetCode4,
    ClaimableBalanceId::ClaimableBalanceIdTypeV0, ConfigSettingId, ContractDataDurability, Hash,
    LedgerKey, LedgerKeyAccount, LedgerKeyClaimableBalance, LedgerKeyConfigSetting,
    LedgerKeyContractCode, LedgerKeyContractData, LedgerKeyData, LedgerKeyLiquidityPool,
    LedgerKeyOffer, LedgerKeyTrustLine, LedgerKeyTtl, Limits, MuxedAccount, PoolId, PublicKey,
    ReadXdr, ScAddress, ScVal, String64, TrustLineAsset, Uint256,
};

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub network: network::Args,

    #[command(flatten)]
    pub locator: locator::Args,

    /// Name of identity to lookup, default is test identity
    pub account: String,
    
    /// If identity is a seed phrase use this hd path, default is 0
    #[arg(long)]
    pub hd_path: Option<usize>,
     
    /// Format of the output
    #[arg(long, default_value = "json")]
    pub output: OutputFormat,
}


//     /// Assets to get trustline info for
//     #[arg(long)]
//     pub asset: Option<Vec<String>>,
//     /// ID of an offer made on the Stellar DEX
//     #[arg(long)]
//     pub offer: Option<Vec<i64>>,
//     /// Fetch key-value data entries attached to an account (see manageDataOp)
//     #[arg(long)]
//     pub data_name: Option<Vec<String>>,

//     /// Claimable Balance id
//     #[arg(long)]
//     pub claimable_id: Option<Vec<String>>,

//     /// Liquidity pool id
//     #[arg(long)]
//     pub pool_id: Option<Vec<String>>,

//     /// Defines the currently active network configuration
//     #[arg(long)]
//     pub config_setting_id: Option<Vec<i32>>,

//     /// Get WASM bytecode by hash
//     #[arg(long)]
//     pub wasm_hash: Option<Vec<String>>,

//     /// Get the time-to-live of an associated contract data or code entry
//     #[arg(long)]
//     pub ttl: Option<Vec<String>>,

//     /// Contract id to fetch an info for
//     #[arg(long = "contract-id", env = "STELLAR_CONTRACT_ID")]
//     pub contract_id: Option<config::UnresolvedContract>,
//     /// Storage entry durability
//     #[arg(long, value_enum, default_value = "persistent")]
//     pub durability: Durability,
//     /// Storage key (symbols only)
//     #[arg(long = "key")]
//     pub key: Option<Vec<String>>,
//     /// Storage key (base64-encoded XDR)
//     #[arg(long = "key-xdr")]
//     pub key_xdr: Option<Vec<String>>,

// }

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Config(#[from] config::key::Error),
    #[error(transparent)]
    Locator(#[from] locator::Error),
    #[error(transparent)]
    Network(#[from] network::Error),
    #[error(transparent)]
    Rpc(#[from] rpc::Error),
    #[error(transparent)]
    Serde(#[from] serde_json::Error),
} 

//     #[error(transparent)]
//     StellarXdr(#[from] stellar_xdr::curr::Error),
//     #[error(transparent)]
//     Spec(#[from] soroban_spec_tools::Error),
//     #[error(transparent)]
//     TryFromSliceError(#[from] TryFromSliceError),
//     #[error(transparent)]
//     FromHexError(#[from] FromHexError),
//     #[error("at least one key must be provided")]
//     EmptyKeys,
//     #[error("contract id is required but was not provided")]
//     ContractRequired,
//     #[error("account is required but was not provided")]
//     AccountRequired,
//     #[error("provided asset is invalid: {0}")]
//     InvalidAsset(String),
//     #[error("provided data name is invalid: {0}")]
//     InvalidDataName(String),
//     #[error("provided hash value is invalid: {0}")]
//     InvalidHash(String),
//     #[error("provided config id is invalid: {0}")]
//     InvalidConfigId(i32),
// }

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

        self.insert_account_keys(&mut ledger_keys)?;

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
        let acc = self.muxed_account(&self.account)?;
        // always add the account key into the list
        // should we allow this to be configurable?
        let key = LedgerKey::Account(LedgerKeyAccount {
            account_id: acc.account_id(),
        });

        ledger_keys.push(key);


        Ok(())
    }

    fn muxed_account(&self, account: &str) -> Result<MuxedAccount, Error> {
        Ok(self
            .locator
            .read_key(account)?
            .muxed_account(self.hd_path)?)
    }

}