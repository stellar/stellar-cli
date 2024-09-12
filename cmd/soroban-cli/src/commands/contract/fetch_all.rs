use async_compression::tokio::bufread::GzipDecoder;
use bytesize::ByteSize;
use clap::{arg, Parser};
use futures::{StreamExt, TryStreamExt};
use http::Uri;
use humantime::format_duration;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{self},
    path::PathBuf,
    str::FromStr,
    time::{Duration, Instant},
};
use stellar_xdr::curr::{
    self as xdr, BucketEntry, Frame, LedgerEntryData, Limited, Limits, ReadXdr,
};
use tokio::fs::OpenOptions;

use crate::{
    commands::{config::data, global, HEADING_RPC},
    config::{self, locator, network::passphrase},
    print,
};

/// Fetch all contract wasms from a network to disk.
#[derive(Parser, Debug, Clone)]
#[group(skip)]
#[command(arg_required_else_help = true)]
pub struct Cmd {
    /// The ledger sequence number to download from. Defaults to latest history archived ledger.
    #[arg(long)]
    ledger: Option<u32>,
    #[command(flatten)]
    locator: locator::Args,
    #[command(flatten)]
    network: config::network::Args,
    /// Archive URL
    #[arg(long, help_heading = HEADING_RPC, env = "STELLAR_ARCHIVE_URL")]
    archive_url: Option<Uri>,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("downloading history: {0}")]
    DownloadingHistory(hyper::Error),
    #[error("downloading history: got status code {0}")]
    DownloadingHistoryGotStatusCode(hyper::StatusCode),
    #[error("json decoding history: {0}")]
    JsonDecodingHistory(serde_json::Error),
    #[error("opening cached bucket to read: {0}")]
    ReadOpeningCachedBucket(io::Error),
    #[error("parsing bucket url: {0}")]
    ParsingBucketUrl(http::uri::InvalidUri),
    #[error("getting bucket: {0}")]
    GettingBucket(hyper::Error),
    #[error("getting bucket: got status code {0}")]
    GettingBucketGotStatusCode(hyper::StatusCode),
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
    ReadHistoryHttpStream(hyper::Error),
    #[error(transparent)]
    Network(#[from] config::network::Error),
    #[error(transparent)]
    Locator(#[from] locator::Error),
    #[error(transparent)]
    Config(#[from] config::Error),
    #[error("archive url not configured")]
    ArchiveUrlNotConfigured,
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

        {
            print.infoln(format!("Searching for wasms"));

            for (i, bucket) in buckets.iter().enumerate() {
                // Defined where the bucket will be read from, either from cache on
                // disk, or streamed from the archive.
                let cache_path = cache_bucket(&print, &archive_url, i, bucket).await?;
                let file = std::fs::OpenOptions::new()
                    .read(true)
                    .open(&cache_path)
                    .map_err(Error::ReadOpeningCachedBucket)?;

                let message = format!("Searching bucket {i} {bucket}");
                print.search(format!("{message}…"));

                if let Ok(metadata) = file.metadata() {
                    print.clear_line();
                    print.searchln(format!("{message} ({})", ByteSize(metadata.len())));
                }

                // Stream the bucket entries from the bucket, identifying
                // entries that match the filters, and including only the
                // entries that match in the snapshot.
                let limited = &mut Limited::new(file, Limits::none());
                let entries = Frame::<BucketEntry>::read_xdr_iter(limited);
                for entry in entries {
                    let Frame(entry) = entry.map_err(Error::ReadXdrFrameBucketEntry)?;
                    let val = match entry {
                        BucketEntry::Liveentry(l) | BucketEntry::Initentry(l) => l,
                        BucketEntry::Deadentry(_) | BucketEntry::Metaentry(_) => continue,
                    };
                    if let LedgerEntryData::ContractCode(c) = val.data {
                        let hash = c.hash.to_string();
                        let path = format!("{hash}.wasm");
                        let code = c.code.to_vec();
                        print.infoln(format!("Found {hash} ({})", ByteSize(code.len() as u64)));
                        fs::write(path, code).unwrap();
                    };
                }
            }
        }

        let duration = Duration::from_secs(start.elapsed().as_secs());
        print.checkln(format!("Completed in {}", format_duration(duration)));

        Ok(())
    }

    fn archive_url(&self) -> Result<http::Uri, Error> {
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
                    .map(|s| Uri::from_str(s).expect("archive url valid"))
                })
            })
            .ok_or(Error::ArchiveUrlNotConfigured)
    }
}

async fn get_history(
    print: &print::Print,
    archive_url: &Uri,
    ledger: Option<u32>,
) -> Result<History, Error> {
    let archive_url = archive_url.to_string();
    let archive_url = archive_url.strip_suffix('/').unwrap_or(&archive_url);
    let history_url = if let Some(ledger) = ledger {
        let ledger_hex = format!("{ledger:08x}");
        let ledger_hex_0 = &ledger_hex[0..=1];
        let ledger_hex_1 = &ledger_hex[2..=3];
        let ledger_hex_2 = &ledger_hex[4..=5];
        format!("{archive_url}/history/{ledger_hex_0}/{ledger_hex_1}/{ledger_hex_2}/history-{ledger_hex}.json")
    } else {
        format!("{archive_url}/.well-known/stellar-history.json")
    };
    let history_url = Uri::from_str(&history_url).unwrap();

    print.globe(format!("Downloading history {history_url}"));

    let https = hyper_tls::HttpsConnector::new();
    let response = hyper::Client::builder()
        .build::<_, hyper::Body>(https)
        .get(history_url.clone())
        .await
        .map_err(Error::DownloadingHistory)?;

    if !response.status().is_success() {
        // Check ledger is a checkpoint ledger and available in archives.
        if let Some(ledger) = ledger {
            let ledger_offset = (ledger + 1) % CHECKPOINT_FREQUENCY;

            if ledger_offset != 0 {
                print.println("");
                print.errorln(format!(
                    "Ledger {ledger} may not be a checkpoint ledger, try {} or {}",
                    ledger - ledger_offset,
                    ledger + (CHECKPOINT_FREQUENCY - ledger_offset),
                ));
            }
        }
        return Err(Error::DownloadingHistoryGotStatusCode(response.status()));
    }

    let body = hyper::body::to_bytes(response.into_body())
        .await
        .map_err(Error::ReadHistoryHttpStream)?;

    print.clear_line();
    print.globeln(format!("Downloaded history {}", &history_url));

    serde_json::from_slice::<History>(&body).map_err(Error::JsonDecodingHistory)
}

async fn cache_bucket(
    print: &print::Print,
    archive_url: &Uri,
    bucket_index: usize,
    bucket: &str,
) -> Result<PathBuf, Error> {
    let bucket_dir = data::bucket_dir().map_err(Error::GetBucketDir)?;
    let cache_path = bucket_dir.join(format!("bucket-{bucket}.xdr"));
    if !cache_path.exists() {
        let bucket_0 = &bucket[0..=1];
        let bucket_1 = &bucket[2..=3];
        let bucket_2 = &bucket[4..=5];
        let bucket_url =
            format!("{archive_url}/bucket/{bucket_0}/{bucket_1}/{bucket_2}/bucket-{bucket}.xdr.gz");

        print.globe(format!("Downloading bucket {bucket_index} {bucket}…"));

        let bucket_url = Uri::from_str(&bucket_url).map_err(Error::ParsingBucketUrl)?;
        let https = hyper_tls::HttpsConnector::new();
        let response = hyper::Client::builder()
            .build::<_, hyper::Body>(https)
            .get(bucket_url)
            .await
            .map_err(Error::GettingBucket)?;

        if !response.status().is_success() {
            print.println("");
            return Err(Error::GettingBucketGotStatusCode(response.status()));
        }

        if let Some(val) = response.headers().get("Content-Length") {
            if let Ok(str) = val.to_str() {
                if let Ok(len) = str.parse::<u64>() {
                    print.clear_line();
                    print.globe(format!(
                        "Downloaded bucket {bucket_index} {bucket} ({})",
                        ByteSize(len)
                    ));
                }
            }
        }

        print.println("");

        let read = response
            .into_body()
            .map(|result| result.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e)))
            .into_async_read();
        let read = tokio_util::compat::FuturesAsyncReadCompatExt::compat(read);
        let mut read = GzipDecoder::new(read);
        let dl_path = cache_path.with_extension("dl");
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&dl_path)
            .await
            .map_err(Error::WriteOpeningCachedBucket)?;
        tokio::io::copy(&mut read, &mut file)
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
