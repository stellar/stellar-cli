use std::io::Cursor;

use clap::Parser;
use serde::Deserialize;
use stellar_xdr::{LedgerCloseMeta, LedgerCloseMetaBatch, Limits, ReadXdr, WriteXdr};

use crate::{commands::global, config::network, utils::XDR_DEPTH_LIMIT};

pub const PUBNET_DATASTORE_URL: &str =
    "https://aws-public-blockchain.s3.us-east-2.amazonaws.com/v1.1/stellar/ledgers/pubnet";
pub const TESTNET_DATASTORE_URL: &str =
    "https://aws-public-blockchain.s3.us-east-2.amazonaws.com/v1.1/stellar/ledgers/testnet/2025-06-18";
pub const LOCAL_DATASTORE_URL: &str = "http://localhost:8000/ledger-meta";

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, clap::ValueEnum, Default)]
pub enum OutputFormat {
    /// Base64-encoded XDR
    Xdr,
    /// JSON output
    #[default]
    Json,
    /// Formatted (multiline) JSON output
    JsonFormatted,
}

#[derive(Debug, Parser)]
pub struct Cmd {
    /// Fetch metadata for a single ledger sequence
    #[arg(long, conflicts_with_all = ["start", "end"])]
    pub ledger: Option<u32>,

    /// Start ledger sequence for a range fetch (inclusive)
    #[arg(long, requires = "end", conflicts_with = "ledger")]
    pub start: Option<u32>,

    /// End ledger sequence for a range fetch (inclusive)
    #[arg(long, requires = "start", conflicts_with = "ledger")]
    pub end: Option<u32>,

    #[command(flatten)]
    pub network: network::Args,

    /// Custom SEP-54 datastore URL (overrides default network datastore URL)
    #[arg(long)]
    pub datastore_url: Option<String>,

    /// Format of the output
    #[arg(long, value_enum, default_value_t)]
    pub output: OutputFormat,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("either --ledger or both --start and --end must be provided")]
    MissingRangeOrLedger,

    #[error("--end ({end}) must be greater than or equal to --start ({start})")]
    InvalidRange { start: u32, end: u32 },

    #[error("network request failed for {url}: {source}")]
    Reqwest { url: String, source: reqwest::Error },

    #[error("HTTP error {status} when fetching {url}")]
    HttpStatus {
        url: String,
        status: reqwest::StatusCode,
    },

    #[error("failed to decompress batch with zstd: {0}")]
    Zstd(std::io::Error),

    #[error(transparent)]
    Xdr(#[from] stellar_xdr::Error),

    #[error(transparent)]
    Serde(#[from] serde_json::Error),

    #[error(transparent)]
    Network(#[from] network::Error),

    #[error("ledger {0} not found in fetched batch")]
    LedgerNotFound(u32),

    #[error("cannot determine datastore URL for network; please provide --datastore-url")]
    NoDatastoreUrl,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatastoreConfig {
    pub network_passphrase: Option<String>,
    pub version: Option<String>,
    pub compression: Option<String>,
    pub ledgers_per_batch: u32,
    pub batches_per_partition: u32,
}

/// Compute partition and batch key path according to SEP-54
pub fn compute_batch_path(seq: u32, ledgers_per_batch: u32, batches_per_partition: u32) -> String {
    let ledgers_per_batch = ledgers_per_batch.max(1);
    let batches_per_partition = batches_per_partition.max(1);
    let ledgers_per_partition = ledgers_per_batch.saturating_mul(batches_per_partition);

    let batch_index = seq / ledgers_per_batch;
    let batch_start = batch_index * ledgers_per_batch;
    let batch_end = batch_start + ledgers_per_batch - 1;

    let batch_filename = if ledgers_per_batch == 1 {
        format!("{:08X}--{batch_start}.xdr.zst", u32::MAX - batch_start)
    } else {
        format!(
            "{:08X}--{batch_start}-{batch_end}.xdr.zst",
            u32::MAX - batch_start
        )
    };

    if batches_per_partition <= 1 {
        batch_filename
    } else {
        let partition_index = seq / ledgers_per_partition;
        let partition_start = partition_index * ledgers_per_partition;
        let partition_end = partition_start + ledgers_per_partition - 1;
        let partition_dir = format!(
            "{:08X}--{partition_start}-{partition_end}",
            u32::MAX - partition_start
        );
        format!("{partition_dir}/{batch_filename}")
    }
}

pub fn get_ledger_sequence(meta: &LedgerCloseMeta) -> u32 {
    match meta {
        LedgerCloseMeta::V0(v) => v.ledger_header.header.ledger_seq,
        LedgerCloseMeta::V1(v) => v.ledger_header.header.ledger_seq,
        LedgerCloseMeta::V2(v) => v.ledger_header.header.ledger_seq,
    }
}

pub fn resolve_datastore_url(
    cmd_url: Option<&str>,
    network_name: Option<&str>,
    network_passphrase: Option<&str>,
) -> Result<String, Error> {
    if let Some(url) = cmd_url {
        return Ok(url.trim_end_matches('/').to_string());
    }

    if let Some(name) = network_name {
        let lower = name.to_ascii_lowercase();
        if lower == "pubnet" || lower == "mainnet" {
            return Ok(PUBNET_DATASTORE_URL.to_string());
        }
        if lower == "testnet" {
            return Ok(TESTNET_DATASTORE_URL.to_string());
        }
        if lower == "local" || lower == "standalone" {
            return Ok(LOCAL_DATASTORE_URL.to_string());
        }
    }

    if let Some(passphrase) = network_passphrase {
        if passphrase == "Public Global Stellar Network ; September 2015" {
            return Ok(PUBNET_DATASTORE_URL.to_string());
        }
        if passphrase == "Test SDF Network ; September 2015" {
            return Ok(TESTNET_DATASTORE_URL.to_string());
        }
        if passphrase == "Standalone Network ; February 2017" {
            return Ok(LOCAL_DATASTORE_URL.to_string());
        }
    }

    Err(Error::NoDatastoreUrl)
}

pub async fn fetch_datastore_config(
    client: &reqwest::Client,
    base_url: &str,
) -> Result<DatastoreConfig, Error> {
    let url = format!("{base_url}/.config.json");
    let resp = client
        .get(&url)
        .header("User-Agent", "stellar-cli")
        .send()
        .await
        .map_err(|source| Error::Reqwest {
            url: url.clone(),
            source,
        })?;

    if !resp.status().is_success() {
        return Err(Error::HttpStatus {
            url,
            status: resp.status(),
        });
    }

    let config: DatastoreConfig = resp
        .json()
        .await
        .map_err(|source| Error::Reqwest { url, source })?;
    Ok(config)
}

pub async fn fetch_and_decompress_batch(
    client: &reqwest::Client,
    base_url: &str,
    batch_path: &str,
) -> Result<LedgerCloseMetaBatch, Error> {
    let url = format!("{base_url}/{batch_path}");
    let resp = client
        .get(&url)
        .header("User-Agent", "stellar-cli")
        .send()
        .await
        .map_err(|source| Error::Reqwest {
            url: url.clone(),
            source,
        })?;

    if !resp.status().is_success() {
        return Err(Error::HttpStatus {
            url,
            status: resp.status(),
        });
    }

    let bytes = resp.bytes().await.map_err(|source| Error::Reqwest {
        url: url.clone(),
        source,
    })?;

    let decompressed = zstd::decode_all(Cursor::new(&bytes)).map_err(Error::Zstd)?;

    let limits = Limits {
        depth: XDR_DEPTH_LIMIT,
        len: usize::MAX,
    };
    let batch = LedgerCloseMetaBatch::from_xdr(&decompressed, limits)?;
    Ok(batch)
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let (start, end, single_ledger_mode) = match (self.ledger, self.start, self.end) {
            (Some(l), None, None) => (l, l, true),
            (None, Some(s), Some(e)) => {
                if e < s {
                    return Err(Error::InvalidRange { start: s, end: e });
                }
                (s, e, false)
            }
            _ => return Err(Error::MissingRangeOrLedger),
        };

        let network_name = self.network.network.as_deref();
        let network_passphrase = self
            .network
            .get(&global_args.locator)
            .ok()
            .map(|net| net.network_passphrase)
            .or_else(|| self.network.network_passphrase.clone());

        let base_url = resolve_datastore_url(
            self.datastore_url.as_deref(),
            network_name,
            network_passphrase.as_deref(),
        )?;

        let client = reqwest::Client::new();
        let config = fetch_datastore_config(&client, &base_url).await?;

        let ledgers_per_batch = config.ledgers_per_batch.max(1);
        let first_batch_idx = start / ledgers_per_batch;
        let last_batch_idx = end / ledgers_per_batch;

        let mut collected_metas = Vec::new();

        for batch_idx in first_batch_idx..=last_batch_idx {
            let batch_seq = batch_idx * ledgers_per_batch;
            let batch_path = compute_batch_path(
                batch_seq,
                config.ledgers_per_batch,
                config.batches_per_partition,
            );

            let batch = fetch_and_decompress_batch(&client, &base_url, &batch_path).await?;

            for meta in batch.ledger_close_metas {
                let seq = get_ledger_sequence(&meta);
                if seq >= start && seq <= end {
                    collected_metas.push(meta);
                }
            }
        }

        let limits = Limits {
            depth: XDR_DEPTH_LIMIT,
            len: usize::MAX,
        };

        if single_ledger_mode {
            let single_meta = collected_metas
                .into_iter()
                .find(|m| get_ledger_sequence(m) == start)
                .ok_or(Error::LedgerNotFound(start))?;

            match self.output {
                OutputFormat::Xdr => {
                    println!("{}", single_meta.to_xdr_base64(limits)?);
                }
                OutputFormat::Json => {
                    println!("{}", serde_json::to_string(&single_meta)?);
                }
                OutputFormat::JsonFormatted => {
                    println!("{}", serde_json::to_string_pretty(&single_meta)?);
                }
            }
        } else {
            let reassembled_batch = LedgerCloseMetaBatch {
                start_sequence: start,
                end_sequence: end,
                ledger_close_metas: collected_metas.try_into()?,
            };

            match self.output {
                OutputFormat::Xdr => {
                    println!("{}", reassembled_batch.to_xdr_base64(limits)?);
                }
                OutputFormat::Json => {
                    println!("{}", serde_json::to_string(&reassembled_batch)?);
                }
                OutputFormat::JsonFormatted => {
                    println!("{}", serde_json::to_string_pretty(&reassembled_batch)?);
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_batch_path_pubnet() {
        // Pubnet config: ledgersPerBatch = 1, batchesPerPartition = 64000
        let ledgers_per_batch = 1;
        let batches_per_partition = 64000;

        let path = compute_batch_path(65090997, ledgers_per_batch, batches_per_partition);
        assert_eq!(
            path,
            "FC1ED5FF--65088000-65151999/FC1ECA4A--65090997.xdr.zst"
        );

        let path_0 = compute_batch_path(0, ledgers_per_batch, batches_per_partition);
        assert_eq!(path_0, "FFFFFFFF--0-63999/FFFFFFFF--0.xdr.zst");

        let path_1000 = compute_batch_path(1000, ledgers_per_batch, batches_per_partition);
        assert_eq!(path_1000, "FFFFFFFF--0-63999/FFFFFC17--1000.xdr.zst");
    }

    #[test]
    fn test_compute_batch_path_multi_ledger_batch() {
        // Multi-ledger batch: ledgersPerBatch = 2, batchesPerPartition = 8 (from SEP-54 example)
        let ledgers_per_batch = 2;
        let batches_per_partition = 8;

        let path_2 = compute_batch_path(2, ledgers_per_batch, batches_per_partition);
        assert_eq!(path_2, "FFFFFFFF--0-15/FFFFFFFD--2-3.xdr.zst");

        let path_18 = compute_batch_path(18, ledgers_per_batch, batches_per_partition);
        assert_eq!(path_18, "FFFFFFEF--16-31/FFFFFFED--18-19.xdr.zst");
    }

    #[test]
    fn test_compute_batch_path_partition_size_one() {
        // When batchesPerPartition == 1, partition is omitted
        let path = compute_batch_path(100, 1, 1);
        assert_eq!(path, "FFFFFF9B--100.xdr.zst");
    }

    #[test]
    fn test_resolve_datastore_url() {
        let pubnet = resolve_datastore_url(
            None,
            None,
            Some("Public Global Stellar Network ; September 2015"),
        )
        .unwrap();
        assert_eq!(pubnet, PUBNET_DATASTORE_URL);

        let pubnet_by_name = resolve_datastore_url(None, Some("pubnet"), None).unwrap();
        assert_eq!(pubnet_by_name, PUBNET_DATASTORE_URL);

        let testnet =
            resolve_datastore_url(None, None, Some("Test SDF Network ; September 2015")).unwrap();
        assert_eq!(testnet, TESTNET_DATASTORE_URL);

        let custom =
            resolve_datastore_url(Some("https://example.com/datastore/"), None, None).unwrap();
        assert_eq!(custom, "https://example.com/datastore");

        let err = resolve_datastore_url(None, None, None);
        assert!(err.is_err());
    }

    #[test]
    fn test_batch_compression_and_extraction() {
        use stellar_xdr::{LedgerCloseMetaV0, LedgerHeader, LedgerHeaderHistoryEntry};

        let header = LedgerHeader {
            ledger_seq: 100,
            ..Default::default()
        };
        let header_entry = LedgerHeaderHistoryEntry {
            header,
            ..Default::default()
        };
        let meta_v0 = LedgerCloseMetaV0 {
            ledger_header: header_entry,
            tx_set: Default::default(),
            tx_processing: vec![].try_into().unwrap(),
            upgrades_processing: vec![].try_into().unwrap(),
            scp_info: vec![].try_into().unwrap(),
        };
        let meta = LedgerCloseMeta::V0(meta_v0);

        assert_eq!(get_ledger_sequence(&meta), 100);

        let batch = LedgerCloseMetaBatch {
            start_sequence: 100,
            end_sequence: 100,
            ledger_close_metas: vec![meta.clone()].try_into().unwrap(),
        };

        let limits = Limits::none();
        let xdr_bytes = batch.to_xdr(limits.clone()).unwrap();
        let compressed = zstd::encode_all(Cursor::new(&xdr_bytes), 3).unwrap();

        let decompressed = zstd::decode_all(Cursor::new(&compressed)).unwrap();
        let decoded_batch = LedgerCloseMetaBatch::from_xdr(&decompressed, limits).unwrap();
        assert_eq!(decoded_batch.start_sequence, 100);
        assert_eq!(decoded_batch.end_sequence, 100);
        assert_eq!(decoded_batch.ledger_close_metas.len(), 1);
        assert_eq!(
            get_ledger_sequence(&decoded_batch.ledger_close_metas[0]),
            100
        );
    }
}
