use bytesize::ByteSize;
use clap::{arg, Parser};
use flate2::bufread::GzDecoder;
use futures::TryStreamExt;
use http::Uri;
use soroban_ledger_snapshot::LedgerSnapshot;
use std::{
    collections::HashSet,
    io::{self, BufReader},
    path::PathBuf,
    str::FromStr,
};
use stellar_xdr::curr::{
    BucketEntry, ConfigSettingEntry, ConfigSettingId, Frame, LedgerEntry, LedgerEntryData,
    LedgerKey, LedgerKeyAccount, LedgerKeyClaimableBalance, LedgerKeyConfigSetting,
    LedgerKeyContractCode, LedgerKeyContractData, LedgerKeyData, LedgerKeyLiquidityPool,
    LedgerKeyOffer, LedgerKeyTrustLine, LedgerKeyTtl, Limited, Limits, ReadXdr,
};
use tokio_util::compat::FuturesAsyncReadCompatExt as _;

use soroban_env_host::xdr::{self};

use super::{
    config::{self, locator},
    network,
};
use crate::rpc;

fn default_out_path() -> PathBuf {
    PathBuf::new().join("snapshot.json")
}

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    /// The ledger sequence number to snapshot.
    #[arg(long)]
    ledger: u32,
    /// The out path that the snapshot is written to.
    #[arg(long, default_value=default_out_path().into_os_string())]
    out: PathBuf,
    /// Account IDs to filter by.
    #[arg(long = "account-id", help_heading = "FILTERS")]
    account_ids: Vec<String>,
    /// Contract IDs to filter by.
    #[arg(long = "contract-id", help_heading = "FILTERS")]
    contract_ids: Vec<String>,
    /// Contract IDs to filter by.
    #[arg(long = "wasm-hash", help_heading = "FILTERS")]
    wasm_hashes: Vec<String>,
    // #[command(flatten)]
    // locator: locator::Args,
    // #[command(flatten)]
    // network: network::Args,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("cursor is not valid")]
    InvalidCursor,
    #[error("filepath does not exist: {path}")]
    InvalidFile { path: String },
    #[error("filepath ({path}) cannot be read: {error}")]
    CannotReadFile { path: String, error: String },
    #[error("cannot parse topic filter {topic} into 1-4 segments")]
    InvalidTopicFilter { topic: String },
    #[error("invalid segment ({segment}) in topic filter ({topic}): {error}")]
    InvalidSegment {
        topic: String,
        segment: String,
        error: xdr::Error,
    },
    #[error("cannot parse contract ID {contract_id}: {error}")]
    InvalidContractId {
        contract_id: String,
        error: stellar_strkey::DecodeError,
    },
    #[error("invalid JSON string: {error} ({debug})")]
    InvalidJson {
        debug: String,
        error: serde_json::Error,
    },
    #[error("invalid timestamp in event: {ts}")]
    InvalidTimestamp { ts: String },
    #[error("missing start_ledger and cursor")]
    MissingStartLedgerAndCursor,
    #[error("missing target")]
    MissingTarget,
    #[error(transparent)]
    Rpc(#[from] rpc::Error),
    #[error(transparent)]
    Generic(#[from] Box<dyn std::error::Error>),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Xdr(#[from] xdr::Error),
    #[error(transparent)]
    Serde(#[from] serde_json::Error),
    #[error(transparent)]
    Network(#[from] network::Error),
    #[error(transparent)]
    Locator(#[from] locator::Error),
    #[error(transparent)]
    Config(#[from] config::Error),
}

impl Cmd {
    pub async fn run(&self) -> Result<(), Error> {
        const BASE_URL: &str = "http://history.stellar.org/prd/core-live/core_live_001";
        let ledger = self.ledger;

        let ledger_offset = (ledger + 1) % 64;
        if ledger_offset != 0 {
            println!(
                "ledger {ledger} not a checkpoint ledger, use {} or {}",
                ledger - ledger_offset,
                ledger + (64 - ledger_offset),
            );
            return Ok(());
        }

        let ledger_hex = format!("{ledger:08x}");
        let ledger_hex_0 = &ledger_hex[0..=1];
        let ledger_hex_1 = &ledger_hex[2..=3];
        let ledger_hex_2 = &ledger_hex[4..=5];
        let history_url = format!("{BASE_URL}/history/{ledger_hex_0}/{ledger_hex_1}/{ledger_hex_2}/history-{ledger_hex}.json");
        let history_url = Uri::from_str(&history_url).unwrap();
        println!("🌎 Downloading history {history_url}");

        let https = hyper_tls::HttpsConnector::new();
        let response = hyper::Client::builder()
            .build::<_, hyper::Body>(https)
            .get(history_url)
            .await
            .unwrap();
        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();

        let history = serde_json::from_slice::<History>(&body).unwrap();

        let buckets = history
            .current_buckets
            .iter()
            .flat_map(|h| [h.curr.clone(), h.snap.clone()])
            .filter(|b| b != "0000000000000000000000000000000000000000000000000000000000000000")
            .collect::<Vec<_>>();

        let mut seen = HashSet::<LedgerKey>::new();
        let mut snapshot = LedgerSnapshot {
            protocol_version: 0,
            sequence_number: ledger,
            timestamp: 0,
            network_id: [0u8; 32],
            base_reserve: 1,
            min_persistent_entry_ttl: 0,
            min_temp_entry_ttl: 0,
            max_entry_ttl: 0,
            ledger_entries: Vec::new(),
        };

        for (i, bucket) in buckets.iter().enumerate() {
            let bucket_0 = &bucket[0..=1];
            let bucket_1 = &bucket[2..=3];
            let bucket_2 = &bucket[4..=5];
            let bucket_url = format!(
                "{BASE_URL}/bucket/{bucket_0}/{bucket_1}/{bucket_2}/bucket-{bucket}.xdr.gz"
            );
            print!("🪣  Downloading bucket {i} {bucket_url}");
            let bucket_url = Uri::from_str(&bucket_url).unwrap();

            let https = hyper_tls::HttpsConnector::new();
            let response = hyper::Client::builder()
                .build::<_, hyper::Body>(https)
                .get(bucket_url)
                .await
                .unwrap();

            if let Some(val) = response.headers().get("Content-Length") {
                if let Ok(str) = val.to_str() {
                    if let Ok(len) = str.parse::<u64>() {
                        print!(" ({})", ByteSize(len));
                    }
                }
            }
            println!();

            let read = tokio_util::io::SyncIoBridge::new(
                response
                    .into_body()
                    .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
                    .into_async_read()
                    .compat(),
            );

            let account_ids = self.account_ids.clone();
            let contract_ids = self.contract_ids.clone();
            let wasm_hashes = self.wasm_hashes.clone();
            (seen, snapshot) = tokio::task::spawn_blocking(move || {
                let buf = BufReader::new(read);
                let gz = GzDecoder::new(buf);
                let buf = BufReader::new(gz);
                let limited = &mut Limited::new(buf, Limits::none());
                let sz = Frame::<BucketEntry>::read_xdr_iter(limited);
                let mut count_saved = 0;
                for entry in sz {
                    let Frame(entry) = entry.unwrap();
                    let (key, val) = match entry {
                        BucketEntry::Liveentry(l) | BucketEntry::Initentry(l) => {
                            let k = data_into_key(&l);
                            (k, Some(l))
                        }
                        BucketEntry::Deadentry(k) => (k, None),
                        BucketEntry::Metaentry(m) => {
                            snapshot.protocol_version = m.ledger_version;
                            continue;
                        }
                    };
                    if seen.contains(&key) {
                        continue;
                    }
                    if let Some(val) = val {
                        let keep = match &val.data {
                            LedgerEntryData::Account(e) => {
                                account_ids.contains(&e.account_id.to_string())
                            }
                            LedgerEntryData::Trustline(e) => {
                                account_ids.contains(&e.account_id.to_string())
                            }
                            LedgerEntryData::ContractData(e) => {
                                contract_ids.contains(&e.contract.to_string())
                            }
                            LedgerEntryData::ContractCode(e) => {
                                let hash = hex::encode(e.hash.0);
                                wasm_hashes.contains(&hash)
                            }
                            LedgerEntryData::Offer(_)
                            | LedgerEntryData::Data(_)
                            | LedgerEntryData::ClaimableBalance(_)
                            | LedgerEntryData::LiquidityPool(_)
                            | LedgerEntryData::ConfigSetting(_)
                            | LedgerEntryData::Ttl(_) => false,
                        };
                        seen.insert(key.clone());
                        if keep {
                            snapshot
                                .ledger_entries
                                .push((Box::new(key), (Box::new(val), None)));
                            count_saved += 1;
                        }
                    }
                }
                if count_saved > 0 {
                    println!("🔎 Found {count_saved} entries");
                }
                (seen, snapshot)
            })
            .await
            .unwrap();
        }

        snapshot.write_file(&self.out).unwrap();
        println!(
            "💾 Saved {} entries to {:?}",
            snapshot.ledger_entries.len(),
            self.out
        );

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct History {
    current_buckets: Vec<HistoryBucket>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct HistoryBucket {
    curr: String,
    snap: String,
}

fn data_into_key(d: &LedgerEntry) -> LedgerKey {
    // TODO: Move this function into stellar-xdr.
    match &d.data {
        LedgerEntryData::Account(e) => LedgerKey::Account(LedgerKeyAccount {
            account_id: e.account_id.clone(),
        }),
        LedgerEntryData::Trustline(e) => LedgerKey::Trustline(LedgerKeyTrustLine {
            account_id: e.account_id.clone(),
            asset: e.asset.clone(),
        }),
        LedgerEntryData::Offer(e) => LedgerKey::Offer(LedgerKeyOffer {
            seller_id: e.seller_id.clone(),
            offer_id: e.offer_id,
        }),
        LedgerEntryData::Data(e) => LedgerKey::Data(LedgerKeyData {
            account_id: e.account_id.clone(),
            data_name: e.data_name.clone(),
        }),
        LedgerEntryData::ClaimableBalance(e) => {
            LedgerKey::ClaimableBalance(LedgerKeyClaimableBalance {
                balance_id: e.balance_id.clone(),
            })
        }
        LedgerEntryData::LiquidityPool(e) => LedgerKey::LiquidityPool(LedgerKeyLiquidityPool {
            liquidity_pool_id: e.liquidity_pool_id.clone(),
        }),
        LedgerEntryData::ContractData(e) => LedgerKey::ContractData(LedgerKeyContractData {
            contract: e.contract.clone(),
            key: e.key.clone(),
            durability: e.durability,
        }),
        LedgerEntryData::ContractCode(e) => LedgerKey::ContractCode(LedgerKeyContractCode {
            hash: e.hash.clone(),
        }),
        LedgerEntryData::ConfigSetting(e) => LedgerKey::ConfigSetting(LedgerKeyConfigSetting {
            config_setting_id: match e {
                ConfigSettingEntry::ContractMaxSizeBytes(_) => {
                    ConfigSettingId::ContractMaxSizeBytes
                }
                ConfigSettingEntry::ContractComputeV0(_) => ConfigSettingId::ContractComputeV0,
                ConfigSettingEntry::ContractLedgerCostV0(_) => {
                    ConfigSettingId::ContractLedgerCostV0
                }
                ConfigSettingEntry::ContractHistoricalDataV0(_) => {
                    ConfigSettingId::ContractHistoricalDataV0
                }
                ConfigSettingEntry::ContractEventsV0(_) => ConfigSettingId::ContractEventsV0,
                ConfigSettingEntry::ContractBandwidthV0(_) => ConfigSettingId::ContractBandwidthV0,
                ConfigSettingEntry::ContractCostParamsCpuInstructions(_) => {
                    ConfigSettingId::ContractCostParamsCpuInstructions
                }
                ConfigSettingEntry::ContractCostParamsMemoryBytes(_) => {
                    ConfigSettingId::ContractCostParamsMemoryBytes
                }
                ConfigSettingEntry::ContractDataKeySizeBytes(_) => {
                    ConfigSettingId::ContractDataKeySizeBytes
                }
                ConfigSettingEntry::ContractDataEntrySizeBytes(_) => {
                    ConfigSettingId::ContractDataEntrySizeBytes
                }
                ConfigSettingEntry::StateArchival(_) => ConfigSettingId::StateArchival,
                ConfigSettingEntry::ContractExecutionLanes(_) => {
                    ConfigSettingId::ContractExecutionLanes
                }
                ConfigSettingEntry::BucketlistSizeWindow(_) => {
                    ConfigSettingId::BucketlistSizeWindow
                }
                ConfigSettingEntry::EvictionIterator(_) => ConfigSettingId::EvictionIterator,
            },
        }),
        LedgerEntryData::Ttl(e) => LedgerKey::Ttl(LedgerKeyTtl {
            key_hash: e.key_hash.clone(),
        }),
    }
}
