use async_compression::tokio::bufread::GzipDecoder;
use bytesize::ByteSize;
use clap::{arg, Parser, ValueEnum};
use futures::StreamExt;
use humantime::format_duration;
use itertools::{Either, Itertools};
use sha2::{Digest, Sha256};
use soroban_ledger_snapshot::LedgerSnapshot;
use std::{
    collections::HashSet,
    fs,
    io::{self},
    path::PathBuf,
    str::FromStr,
    time::{Duration, Instant},
};
use stellar_xdr::curr::{
    self as xdr, AccountId, Asset, BucketEntry, ConfigSettingEntry, ConfigSettingId,
    ContractExecutable, Frame, Hash, LedgerEntry, LedgerEntryData, LedgerHeaderHistoryEntry,
    LedgerKey, LedgerKeyAccount, LedgerKeyClaimableBalance, LedgerKeyConfigSetting,
    LedgerKeyContractCode, LedgerKeyContractData, LedgerKeyData, LedgerKeyLiquidityPool,
    LedgerKeyOffer, LedgerKeyTrustLine, LedgerKeyTtl, Limited, Limits, ReadXdr, ScAddress,
    ScContractInstance, ScVal,
};
use tokio::fs::OpenOptions;
use tokio::io::BufReader;
use tokio_util::io::StreamReader;
use url::Url;

use crate::{
    commands::{config::data, global, HEADING_ARCHIVE},
    config::{self, locator, network::passphrase},
    print,
    tx::builder,
    utils::get_name_from_stellar_asset_contract_storage,
};
use crate::{config::address::UnresolvedMuxedAccount, utils::http};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, ValueEnum, Default)]
pub enum Output {
    #[default]
    Json,
}

fn default_out_path() -> PathBuf {
    PathBuf::new().join("snapshot.json")
}

/// Create a ledger snapshot using a history archive.
///
/// Filters (address, wasm-hash) specify what ledger entries to include.
///
/// Account addresses include the account, and trustlines.
///
/// Contract addresses include the related wasm, contract data.
///
/// If a contract is a Stellar asset contract, it includes the asset issuer's
/// account and trust lines, but does not include all the trust lines of other
/// accounts holding the asset. To include them specify the addresses of
/// relevant accounts.
///
/// Any invalid contract id passed as `--address` will be ignored.
///
#[derive(Parser, Debug, Clone)]
#[group(skip)]
#[command(arg_required_else_help = true)]
pub struct Cmd {
    /// The ledger sequence number to snapshot. Defaults to latest history archived ledger.
    #[arg(long)]
    ledger: Option<u32>,

    /// Account or contract address/alias to include in the snapshot.
    #[arg(long = "address", help_heading = "Filter Options")]
    address: Vec<String>,

    /// WASM hashes to include in the snapshot.
    #[arg(long = "wasm-hash", help_heading = "Filter Options")]
    wasm_hashes: Vec<Hash>,

    /// Format of the out file.
    #[arg(long)]
    output: Output,

    /// Out path that the snapshot is written to.
    #[arg(long, default_value=default_out_path().into_os_string())]
    out: PathBuf,

    /// Archive URL
    #[arg(long, help_heading = HEADING_ARCHIVE, env = "STELLAR_ARCHIVE_URL")]
    archive_url: Option<Url>,

    #[command(flatten)]
    locator: locator::Args,

    #[command(flatten)]
    network: config::network::Args,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("wasm hash invalid: {0}")]
    WasmHashInvalid(String),

    #[error("downloading history: {0}")]
    DownloadingHistory(reqwest::Error),

    #[error("downloading history: got status code {0}")]
    DownloadingHistoryGotStatusCode(reqwest::StatusCode),

    #[error("json decoding history: {0}")]
    JsonDecodingHistory(serde_json::Error),

    #[error("opening cached bucket to read: {0}")]
    ReadOpeningCachedBucket(io::Error),

    #[error("parsing bucket url: {0}")]
    ParsingBucketUrl(url::ParseError),

    #[error("getting bucket: {0}")]
    GettingBucket(reqwest::Error),

    #[error("getting bucket: got status code {0}")]
    GettingBucketGotStatusCode(reqwest::StatusCode),

    #[error("opening cached bucket to write: {0}")]
    WriteOpeningCachedBucket(io::Error),

    #[error("streaming bucket: {0}")]
    StreamingBucket(io::Error),

    #[error("read XDR frame bucket entry: {0}")]
    ReadXdrFrameBucketEntry(xdr::Error),

    #[error("renaming temporary downloaded file to final destination: {0}")]
    RenameDownloadFile(io::Error),

    #[error("getting bucket directory: {0}")]
    GetBucketDir(data::Error),

    #[error("reading history http stream: {0}")]
    ReadHistoryHttpStream(reqwest::Error),

    #[error("writing ledger snapshot: {0}")]
    WriteLedgerSnapshot(soroban_ledger_snapshot::Error),

    #[error(transparent)]
    Join(#[from] tokio::task::JoinError),

    #[error(transparent)]
    Network(#[from] config::network::Error),

    #[error(transparent)]
    Locator(#[from] locator::Error),

    #[error(transparent)]
    Config(#[from] config::Error),

    #[error("archive url not configured")]
    ArchiveUrlNotConfigured,

    #[error("parsing asset name: {0}")]
    ParseAssetName(String),

    #[error(transparent)]
    Asset(#[from] builder::asset::Error),

    #[error("ledger not found in archive")]
    LedgerNotFound,

    #[error("xdr parsing error: {0}")]
    Xdr(#[from] xdr::Error),

    #[error("corrupted bucket file: expected hash {expected}, got {actual}")]
    CorruptedBucket { expected: String, actual: String },
}

/// Checkpoint frequency is usually 64 ledgers, but in local test nets it'll
/// often by 8. There's no way to simply detect what frequency to expect ledgers
/// at, so it is hardcoded at 64, and this value is used only to help the user
/// select good ledger numbers when they select one that doesn't exist.
const CHECKPOINT_FREQUENCY: u32 = 64;

impl Cmd {
    #[allow(clippy::too_many_lines)]
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let print = print::Print::new(global_args.quiet);
        let start = Instant::now();

        let archive_url = self.archive_url()?;
        let history = get_history(&print, &archive_url, self.ledger).await?;

        let ledger = history.current_ledger;
        let network_passphrase = &history.network_passphrase;
        let network_id = Sha256::digest(network_passphrase);

        print.infoln(format!("Ledger: {ledger}"));
        print.infoln(format!("Network Passphrase: {network_passphrase}"));
        print.infoln(format!("Network id: {}", hex::encode(network_id)));

        // Get ledger close time and base reserve from archive
        let (ledger_close_time, base_reserve) =
            match get_ledger_metadata_from_archive(&print, &archive_url, ledger).await {
                Ok((close_time, reserve)) => {
                    print.infoln(format!("Ledger Close Time: {close_time}"));
                    print.infoln(format!("Base Reserve: {reserve}"));
                    (close_time, reserve)
                }
                Err(e) => {
                    print.warnln(format!("Failed to get ledger metadata from archive: {e}"));
                    print.infoln("Using default values: close_time=0, base_reserve=1");
                    (0u64, 1u32) // Default values
                }
            };

        // Prepare a flat list of buckets to read. They'll be ordered by their
        // level so that they can iterated higher level to lower level.
        let buckets = history
            .current_buckets
            .iter()
            .flat_map(|h| [h.curr.clone(), h.snap.clone()])
            .filter(|b| b != "0000000000000000000000000000000000000000000000000000000000000000")
            .collect::<Vec<_>>();

        // Pre-cache the buckets.
        for (i, bucket) in buckets.iter().enumerate() {
            cache_bucket(&print, &archive_url, i, bucket).await?;
        }

        // The snapshot is what will be written to file at the end. Fields will
        // be updated while parsing the history archive.
        let mut snapshot = LedgerSnapshot {
            protocol_version: 0,
            sequence_number: ledger,
            timestamp: ledger_close_time,
            network_id: network_id.into(),
            base_reserve,
            min_persistent_entry_ttl: 0,
            min_temp_entry_ttl: 0,
            max_entry_ttl: 0,
            ledger_entries: Vec::new(),
        };

        // Track ledger keys seen, so that we can ignore old versions of
        // entries. Entries can appear in both higher level and lower level
        // buckets, and to get the latest version of the entry the version in
        // the higher level bucket should be used.
        let mut seen = HashSet::new();

        #[allow(clippy::items_after_statements)]
        #[derive(Default)]
        struct SearchInputs {
            account_ids: HashSet<AccountId>,
            contract_ids: HashSet<ScAddress>,
            wasm_hashes: HashSet<Hash>,
        }
        impl SearchInputs {
            pub fn is_empty(&self) -> bool {
                self.account_ids.is_empty()
                    && self.contract_ids.is_empty()
                    && self.wasm_hashes.is_empty()
            }
        }

        // Search the buckets using the user inputs as the starting inputs.
        let (account_ids, contract_ids): (HashSet<AccountId>, HashSet<ScAddress>) = self
            .address
            .iter()
            .cloned()
            .filter_map(|a| self.resolve_address_sync(&a, network_passphrase))
            .partition_map(|a| a);

        let mut current = SearchInputs {
            account_ids,
            contract_ids,
            wasm_hashes: self.wasm_hashes.iter().cloned().collect(),
        };
        let mut next = SearchInputs::default();

        loop {
            if current.is_empty() {
                break;
            }

            print.infoln(format!(
                "Searching for {} accounts, {} contracts, {} wasms",
                current.account_ids.len(),
                current.contract_ids.len(),
                current.wasm_hashes.len(),
            ));

            for (i, bucket) in buckets.iter().enumerate() {
                // Defined where the bucket will be read from, either from cache on
                // disk, or streamed from the archive.
                let cache_path = cache_bucket(&print, &archive_url, i, bucket).await?;
                let file = std::fs::OpenOptions::new()
                    .read(true)
                    .open(&cache_path)
                    .map_err(Error::ReadOpeningCachedBucket)?;

                let message = format!("Searching bucket {i} {bucket}");
                print.searchln(format!("{message}…"));

                if let Ok(metadata) = file.metadata() {
                    print.clear_previous_line();
                    print.searchln(format!("{message} ({})", ByteSize(metadata.len())));
                }

                // Stream the bucket entries from the bucket, identifying
                // entries that match the filters, and including only the
                // entries that match in the snapshot.
                let limited = &mut Limited::new(file, Limits::none());
                let entries = Frame::<BucketEntry>::read_xdr_iter(limited);
                let mut count_saved = 0;
                for entry in entries {
                    let Frame(entry) = entry.map_err(Error::ReadXdrFrameBucketEntry)?;
                    let (key, val) = match entry {
                        BucketEntry::Liveentry(l) | BucketEntry::Initentry(l) => {
                            let k = data_into_key(&l);
                            (k, Some(l))
                        }
                        BucketEntry::Deadentry(k) => (k, None),
                        BucketEntry::Metaentry(m) => {
                            if m.ledger_version > snapshot.protocol_version {
                                snapshot.protocol_version = m.ledger_version;
                                print.infoln(format!(
                                    "Protocol version: {}",
                                    snapshot.protocol_version
                                ));
                            }
                            continue;
                        }
                    };

                    if seen.contains(&key) {
                        continue;
                    }

                    let keep = match &key {
                        LedgerKey::Account(k) => current.account_ids.contains(&k.account_id),
                        LedgerKey::Trustline(k) => current.account_ids.contains(&k.account_id),
                        LedgerKey::ContractData(k) => current.contract_ids.contains(&k.contract),
                        LedgerKey::ContractCode(e) => current.wasm_hashes.contains(&e.hash),
                        LedgerKey::ConfigSetting(_) => true,
                        _ => false,
                    };

                    if !keep {
                        continue;
                    }

                    seen.insert(key.clone());

                    let Some(val) = val else {
                        continue;
                    };

                    match &val.data {
                        LedgerEntryData::ConfigSetting(ConfigSettingEntry::StateArchival(
                            state_archival,
                        )) => {
                            snapshot.min_persistent_entry_ttl = state_archival.min_persistent_ttl;
                            snapshot.min_temp_entry_ttl = state_archival.min_temporary_ttl;
                            snapshot.max_entry_ttl = state_archival.max_entry_ttl;
                            false
                        }

                        LedgerEntryData::ContractData(e) => {
                            // If a contract instance references contract
                            // executable stored in another ledger entry, add
                            // that ledger entry to the filter so that Wasm for
                            // any filtered contract is collected too in the
                            // second pass.
                            if e.key == ScVal::LedgerKeyContractInstance {
                                match &e.val {
                                    ScVal::ContractInstance(ScContractInstance {
                                        executable: ContractExecutable::Wasm(hash),
                                        ..
                                    }) => {
                                        if !current.wasm_hashes.contains(hash) {
                                            next.wasm_hashes.insert(hash.clone());
                                            print.infoln(format!(
                                                "Adding wasm {} to search",
                                                hex::encode(hash)
                                            ));
                                        }
                                    }
                                    ScVal::ContractInstance(ScContractInstance {
                                        executable: ContractExecutable::StellarAsset,
                                        storage: Some(storage),
                                    }) => {
                                        if let Some(name) =
                                            get_name_from_stellar_asset_contract_storage(storage)
                                        {
                                            let asset: builder::Asset = name.parse()?;
                                            if let Some(issuer) = match asset
                                                .resolve(&global_args.locator)?
                                            {
                                                Asset::Native => None,
                                                Asset::CreditAlphanum4(a4) => Some(a4.issuer),
                                                Asset::CreditAlphanum12(a12) => Some(a12.issuer),
                                            } {
                                                print.infoln(format!(
                                                    "Adding asset issuer {issuer} to search"
                                                ));
                                                next.account_ids.insert(issuer);
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            keep
                        }
                        _ => false,
                    };
                    snapshot
                        .ledger_entries
                        .push((Box::new(key), (Box::new(val), Some(u32::MAX))));
                    count_saved += 1;
                }
                if count_saved > 0 {
                    print.infoln(format!("Found {count_saved} entries"));
                }
            }
            current = next;
            next = SearchInputs::default();
        }

        // Write the snapshot to file.
        snapshot
            .write_file(&self.out)
            .map_err(Error::WriteLedgerSnapshot)?;
        print.saveln(format!(
            "Saved {} entries to {:?}",
            snapshot.ledger_entries.len(),
            self.out
        ));

        let duration = Duration::from_secs(start.elapsed().as_secs());
        print.checkln(format!("Completed in {}", format_duration(duration)));

        Ok(())
    }

    fn archive_url(&self) -> Result<Url, Error> {
        // Return the configured archive URL, or if one is not configured, guess
        // at an appropriate archive URL given the network passphrase.
        self.archive_url
            .clone()
            .or_else(|| {
                self.network.get(&self.locator).ok().and_then(|network| {
                    match network.network_passphrase.as_str() {
                        passphrase::MAINNET => {
                            Some("https://history.stellar.org/prd/core-live/core_live_001")
                        }
                        passphrase::TESTNET => {
                            Some("https://history.stellar.org/prd/core-testnet/core_testnet_001")
                        }
                        passphrase::FUTURENET => Some("https://history-futurenet.stellar.org"),
                        passphrase::LOCAL => Some("http://localhost:8000/archive"),
                        _ => None,
                    }
                    .map(|s| Url::from_str(s).expect("archive url valid"))
                })
            })
            .ok_or(Error::ArchiveUrlNotConfigured)
    }

    fn resolve_address_sync(
        &self,
        address: &str,
        network_passphrase: &str,
    ) -> Option<Either<AccountId, ScAddress>> {
        if let Some(contract) = self.resolve_contract(address, network_passphrase) {
            Some(Either::Right(contract))
        } else {
            self.resolve_account_sync(address).map(Either::Left)
        }
    }

    // Resolve an account address to an account id. The address can be a
    // G-address or a key name (as in `stellar keys address NAME`).
    fn resolve_account_sync(&self, address: &str) -> Option<AccountId> {
        let address: UnresolvedMuxedAccount = address.parse().ok()?;
        let muxed_account = address
            .resolve_muxed_account_sync(&self.locator, None)
            .ok()?;
        Some(muxed_account.account_id())
    }

    // Resolve a contract address to a contract id. The contract can be a
    // C-address or a contract alias.
    fn resolve_contract(&self, address: &str, network_passphrase: &str) -> Option<ScAddress> {
        address.parse().ok().or_else(|| {
            Some(ScAddress::Contract(stellar_xdr::curr::ContractId(
                self.locator
                    .resolve_contract_id(address, network_passphrase)
                    .ok()?
                    .0
                    .into(),
            )))
        })
    }
}

fn ledger_to_path_components(ledger: u32) -> (String, String, String, String) {
    let ledger_hex = format!("{ledger:08x}");
    let ledger_hex_0 = ledger_hex[0..=1].to_string();
    let ledger_hex_1 = ledger_hex[2..=3].to_string();
    let ledger_hex_2 = ledger_hex[4..=5].to_string();
    (ledger_hex, ledger_hex_0, ledger_hex_1, ledger_hex_2)
}

async fn get_history(
    print: &print::Print,
    archive_url: &Url,
    ledger: Option<u32>,
) -> Result<History, Error> {
    let archive_url = archive_url.to_string();
    let archive_url = archive_url.strip_suffix('/').unwrap_or(&archive_url);
    let history_url = if let Some(ledger) = ledger {
        let (ledger_hex, ledger_hex_0, ledger_hex_1, ledger_hex_2) =
            ledger_to_path_components(ledger);
        format!("{archive_url}/history/{ledger_hex_0}/{ledger_hex_1}/{ledger_hex_2}/history-{ledger_hex}.json")
    } else {
        format!("{archive_url}/.well-known/stellar-history.json")
    };
    let history_url = Url::from_str(&history_url).unwrap();

    print.globeln(format!("Downloading history {history_url}"));

    let response = http::client()
        .get(history_url.as_str())
        .send()
        .await
        .map_err(Error::DownloadingHistory)?;

    if !response.status().is_success() {
        // Check ledger is a checkpoint ledger and available in archives.
        if let Some(ledger) = ledger {
            let ledger_offset = (ledger + 1) % CHECKPOINT_FREQUENCY;

            if ledger_offset != 0 {
                print.errorln(format!(
                    "Ledger {ledger} may not be a checkpoint ledger, try {} or {}",
                    ledger - ledger_offset,
                    ledger + (CHECKPOINT_FREQUENCY - ledger_offset),
                ));
            }
        }
        return Err(Error::DownloadingHistoryGotStatusCode(response.status()));
    }

    let body = response
        .bytes()
        .await
        .map_err(Error::ReadHistoryHttpStream)?;

    print.clear_previous_line();
    print.globeln(format!("Downloaded history {}", &history_url));

    serde_json::from_slice::<History>(&body).map_err(Error::JsonDecodingHistory)
}

async fn get_ledger_metadata_from_archive(
    print: &print::Print,
    archive_url: &Url,
    ledger: u32,
) -> Result<(u64, u32), Error> {
    let archive_url = archive_url.to_string();
    let archive_url = archive_url.strip_suffix('/').unwrap_or(&archive_url);

    // Calculate the path to the ledger header file
    let (ledger_hex, ledger_hex_0, ledger_hex_1, ledger_hex_2) = ledger_to_path_components(ledger);
    let ledger_url = format!(
        "{archive_url}/ledger/{ledger_hex_0}/{ledger_hex_1}/{ledger_hex_2}/ledger-{ledger_hex}.xdr.gz"
    );

    print.globeln(format!("Downloading ledger headers {ledger_url}"));

    let ledger_url = Url::from_str(&ledger_url).map_err(Error::ParsingBucketUrl)?;
    let response = http::client()
        .get(ledger_url.as_str())
        .send()
        .await
        .map_err(Error::DownloadingHistory)?;

    if !response.status().is_success() {
        return Err(Error::DownloadingHistoryGotStatusCode(response.status()));
    }

    // Cache the ledger file to disk like bucket files
    let ledger_dir = data::bucket_dir().map_err(Error::GetBucketDir)?;
    let cache_path = ledger_dir.join(format!("ledger-{ledger_hex}.xdr"));
    let dl_path = cache_path.with_extension("dl");

    let stream = response
        .bytes_stream()
        .map(|result| result.map_err(std::io::Error::other));
    let stream_reader = StreamReader::new(stream);
    let buf_reader = BufReader::new(stream_reader);
    let mut decoder = GzipDecoder::new(buf_reader);

    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&dl_path)
        .await
        .map_err(Error::WriteOpeningCachedBucket)?;

    tokio::io::copy(&mut decoder, &mut file)
        .await
        .map_err(Error::StreamingBucket)?;

    fs::rename(&dl_path, &cache_path).map_err(Error::RenameDownloadFile)?;

    print.clear_previous_line();
    print.globeln(format!("Downloaded ledger headers for ledger {ledger}"));

    // Now read the cached file
    let file = std::fs::File::open(&cache_path).map_err(Error::ReadOpeningCachedBucket)?;
    let limited = &mut Limited::new(file, Limits::none());

    // Find the specific ledger header entry we need
    let entries = Frame::<LedgerHeaderHistoryEntry>::read_xdr_iter(limited);
    for entry in entries {
        let Frame(header_entry) = entry.map_err(Error::Xdr)?;

        if header_entry.header.ledger_seq == ledger {
            let close_time = header_entry.header.scp_value.close_time.0;
            let base_reserve = header_entry.header.base_reserve;

            return Ok((close_time, base_reserve));
        }
    }

    Err(Error::LedgerNotFound)
}

fn validate_bucket_hash(cache_path: &PathBuf, expected_hash: &str) -> Result<(), Error> {
    let file = std::fs::File::open(cache_path).map_err(Error::ReadOpeningCachedBucket)?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut std::io::BufReader::new(file), &mut hasher)
        .map_err(Error::ReadOpeningCachedBucket)?;
    let actual_hash = hex::encode(hasher.finalize());

    if actual_hash != expected_hash {
        return Err(Error::CorruptedBucket {
            expected: expected_hash.to_string(),
            actual: actual_hash,
        });
    }

    Ok(())
}

async fn cache_bucket(
    print: &print::Print,
    archive_url: &Url,
    bucket_index: usize,
    bucket: &str,
) -> Result<PathBuf, Error> {
    let bucket_dir = data::bucket_dir().map_err(Error::GetBucketDir)?;
    let cache_path = bucket_dir.join(format!("bucket-{bucket}.xdr"));

    // Validate cached bucket if it exists
    if cache_path.exists() {
        if validate_bucket_hash(&cache_path, bucket).is_err() {
            print.warnln(format!(
                "Cached bucket {bucket} is corrupted, re-downloading"
            ));
            std::fs::remove_file(&cache_path).ok();
        } else {
            return Ok(cache_path);
        }
    }

    if !cache_path.exists() {
        let bucket_0 = &bucket[0..=1];
        let bucket_1 = &bucket[2..=3];
        let bucket_2 = &bucket[4..=5];
        let bucket_url =
            format!("{archive_url}/bucket/{bucket_0}/{bucket_1}/{bucket_2}/bucket-{bucket}.xdr.gz");

        print.globeln(format!("Downloading bucket {bucket_index} {bucket}…"));

        let bucket_url = Url::from_str(&bucket_url).map_err(Error::ParsingBucketUrl)?;

        let response = http::client()
            .get(bucket_url.as_str())
            .send()
            .await
            .map_err(Error::GettingBucket)?;

        if !response.status().is_success() {
            print.println("");
            return Err(Error::GettingBucketGotStatusCode(response.status()));
        }

        if let Some(len) = response.content_length() {
            print.clear_previous_line();
            print.globeln(format!(
                "Downloaded bucket {bucket_index} {bucket} ({})",
                ByteSize(len)
            ));
        }

        let stream = response
            .bytes_stream()
            .map(|result| result.map_err(std::io::Error::other));
        let stream_reader = StreamReader::new(stream);
        let buf_reader = BufReader::new(stream_reader);
        let mut decoder = GzipDecoder::new(buf_reader);
        let dl_path = cache_path.with_extension("dl");
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&dl_path)
            .await
            .map_err(Error::WriteOpeningCachedBucket)?;
        tokio::io::copy(&mut decoder, &mut file)
            .await
            .map_err(Error::StreamingBucket)?;
        fs::rename(&dl_path, &cache_path).map_err(Error::RenameDownloadFile)?;
    }
    Ok(cache_path)
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct History {
    current_ledger: u32,
    current_buckets: Vec<HistoryBucket>,
    network_passphrase: String,
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
                ConfigSettingEntry::EvictionIterator(_) => ConfigSettingId::EvictionIterator,
                ConfigSettingEntry::LiveSorobanStateSizeWindow(_) => {
                    ConfigSettingId::LiveSorobanStateSizeWindow
                }
                ConfigSettingEntry::ContractParallelComputeV0(_) => {
                    ConfigSettingId::ContractParallelComputeV0
                }
                ConfigSettingEntry::ContractLedgerCostExtV0(_) => {
                    ConfigSettingId::ContractLedgerCostExtV0
                }
                ConfigSettingEntry::ScpTiming(_) => ConfigSettingId::ScpTiming,
            },
        }),
        LedgerEntryData::Ttl(e) => LedgerKey::Ttl(LedgerKeyTtl {
            key_hash: e.key_hash.clone(),
        }),
    }
}
