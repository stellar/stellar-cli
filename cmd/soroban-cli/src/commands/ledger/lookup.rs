use std::str::FromStr;

use chrono::{DateTime, TimeZone, Utc};
use serde::Serialize;

use crate::{commands::global, config::network, rpc};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Network(#[from] network::Error),
    #[error(transparent)]
    Rpc(#[from] rpc::Error),
    #[error(transparent)]
    Serde(#[from] serde_json::Error),
    #[error("either --timestamp or --seq must be provided")]
    MissingArgument,
    #[error("failed to parse timestamp '{0}': expected Unix epoch seconds/millis or RFC3339/ISO-8601 string (e.g. '2024-05-01T12:00:00Z')")]
    InvalidTimestamp(String),
    #[error("target timestamp {target_time} is earlier than the oldest ledger available on RPC ({oldest_ledger} closed at {oldest_time})")]
    TimestampEarlierThanOldest {
        target_time: String,
        oldest_ledger: u32,
        oldest_time: String,
    },
    #[error("ledger sequence {0} was not found on RPC (available range: {1} to {2})")]
    LedgerNotFound(u32, u32, u32),
    #[error("RPC returned empty ledger list for sequence {0}")]
    EmptyLedgerList(u32),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, clap::ValueEnum, Default)]
pub enum OutputFormat {
    /// Text output
    #[default]
    Text,
    /// JSON result
    Json,
    /// Formatted (multiline) JSON output
    JsonFormatted,
}

#[derive(Debug, clap::Parser)]
pub struct Cmd {
    /// Timestamp to look up (RFC3339 string e.g. '2024-05-01T12:00:00Z' or Unix epoch seconds).
    /// Returns the nearest ledger sequence closed at or just before this time.
    #[arg(long, conflicts_with = "seq")]
    pub timestamp: Option<String>,

    /// Ledger sequence to look up. Returns the close time for this ledger.
    #[arg(long, conflicts_with = "timestamp")]
    pub seq: Option<u32>,

    #[command(flatten)]
    pub network: network::Args,

    /// Format of the output
    #[arg(long, value_enum, default_value_t)]
    pub output: OutputFormat,
}

#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
pub struct LookupResult {
    pub sequence: u32,
    pub closed_at: String,
    pub timestamp: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let result = match (&self.timestamp, self.seq) {
            (Some(ts_str), None) => {
                let target_sec = parse_timestamp(ts_str)?;
                self.lookup_by_timestamp(global_args, target_sec).await?
            }
            (None, Some(seq)) => self.lookup_by_seq(global_args, seq).await?,
            (None, None) => return Err(Error::MissingArgument),
            (Some(_), Some(_)) => unreachable!("clap conflicts_with handles this"),
        };

        match self.output {
            OutputFormat::Text => {
                println!("Ledger Sequence: {}", result.sequence);
                println!(
                    "Closed At: {} (Unix: {})",
                    result.closed_at, result.timestamp
                );
                if let Some(hash) = &result.hash {
                    println!("Hash: {hash}");
                }
            }
            OutputFormat::Json => println!("{}", serde_json::to_string(&result)?),
            OutputFormat::JsonFormatted => println!("{}", serde_json::to_string_pretty(&result)?),
        }

        Ok(())
    }

    async fn lookup_by_seq(
        &self,
        global_args: &global::Args,
        seq: u32,
    ) -> Result<LookupResult, Error> {
        let network = self.network.get(&global_args.locator)?;
        let client = network.rpc_client()?;
        let start = rpc::LedgerStart::Ledger(seq);
        let res = client.get_ledgers(start, Some(1), None).await?;

        let oldest = res.oldest_ledger;
        let latest = res.latest_ledger;

        let ledger = res
            .ledgers
            .into_iter()
            .find(|l| l.sequence == seq)
            .ok_or(Error::LedgerNotFound(seq, oldest, latest))?;

        let ts_sec = ledger.ledger_close_time.parse::<u64>().unwrap_or(0);
        let closed_at = format_utc_seconds(ts_sec);

        Ok(LookupResult {
            sequence: ledger.sequence,
            closed_at,
            timestamp: ts_sec,
            hash: Some(ledger.hash),
        })
    }

    async fn lookup_by_timestamp(
        &self,
        global_args: &global::Args,
        target_sec: u64,
    ) -> Result<LookupResult, Error> {
        let network = self.network.get(&global_args.locator)?;
        let client = network.rpc_client()?;

        // Initial probe to get oldest/latest ledger window
        let latest_info = client.get_latest_ledger().await?;
        let probe_start = rpc::LedgerStart::Ledger(latest_info.sequence);
        let probe_res = client.get_ledgers(probe_start, Some(1), None).await?;

        let low = probe_res.oldest_ledger;
        let high = probe_res.latest_ledger;

        let oldest_time = probe_res.oldest_ledger_close_time.max(0).cast_unsigned();
        let latest_time = probe_res.latest_ledger_close_time.max(0).cast_unsigned();

        // If target is newer than or equal to latest ledger, return latest
        if target_sec >= latest_time {
            let hash = probe_res.ledgers.first().map(|l| l.hash.clone());
            return Ok(LookupResult {
                sequence: high,
                closed_at: format_utc_seconds(latest_time),
                timestamp: latest_time,
                hash,
            });
        }

        // If target is earlier than oldest ledger available on RPC node
        if target_sec < oldest_time {
            return Err(Error::TimestampEarlierThanOldest {
                target_time: format_utc_seconds(target_sec),
                oldest_ledger: low,
                oldest_time: format_utc_seconds(oldest_time),
            });
        }

        search_ledger_by_time(&client, low, high, oldest_time, latest_time, target_sec).await
    }
}

async fn search_ledger_by_time(
    client: &soroban_rpc::Client,
    mut low: u32,
    mut high: u32,
    mut low_time: u64,
    mut high_time: u64,
    target_sec: u64,
) -> Result<LookupResult, Error> {
    while low < high {
        if high - low <= 10 {
            let batch = client
                .get_ledgers(
                    rpc::LedgerStart::Ledger(low),
                    Some((high - low + 1) as usize),
                    None,
                )
                .await?;

            let best = find_closest_ledger_before_or_at(
                &batch.ledgers,
                |l| l.ledger_close_time.parse::<u64>().unwrap_or(0),
                target_sec,
            );

            if let Some(ledger) = best {
                let ts = ledger.ledger_close_time.parse::<u64>().unwrap_or(0);
                return Ok(LookupResult {
                    sequence: ledger.sequence,
                    closed_at: format_utc_seconds(ts),
                    timestamp: ts,
                    hash: Some(ledger.hash.clone()),
                });
            }

            return Ok(LookupResult {
                sequence: low,
                closed_at: format_utc_seconds(low_time),
                timestamp: low_time,
                hash: None,
            });
        }

        let mid = interpolate_midpoint(low, high, low_time, high_time, target_sec);
        let mid_res = client
            .get_ledgers(rpc::LedgerStart::Ledger(mid), Some(1), None)
            .await?;

        if let Some(mid_ledger) = mid_res.ledgers.first() {
            let mid_time = mid_ledger.ledger_close_time.parse::<u64>().unwrap_or(0);
            if mid_time <= target_sec {
                low = mid_ledger.sequence;
                low_time = mid_time;
            } else {
                high = mid_ledger.sequence.saturating_sub(1);
                high_time = mid_time;
            }
        } else {
            let fallback_mid = low + (high - low) / 2;
            high = fallback_mid;
        }
    }

    let res = client
        .get_ledgers(rpc::LedgerStart::Ledger(low), Some(1), None)
        .await?;
    if let Some(ledger) = res.ledgers.first() {
        let ts = ledger.ledger_close_time.parse::<u64>().unwrap_or(0);
        Ok(LookupResult {
            sequence: ledger.sequence,
            closed_at: format_utc_seconds(ts),
            timestamp: ts,
            hash: Some(ledger.hash.clone()),
        })
    } else {
        Ok(LookupResult {
            sequence: low,
            closed_at: format_utc_seconds(low_time),
            timestamp: low_time,
            hash: None,
        })
    }
}

#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
fn interpolate_midpoint(
    low: u32,
    high: u32,
    low_time: u64,
    high_time: u64,
    target_sec: u64,
) -> u32 {
    let time_span = (high_time.saturating_sub(low_time).max(1) as f64).max(1.0);
    let time_offset = target_sec.saturating_sub(low_time) as f64;
    let fraction = (time_offset / time_span).clamp(0.05, 0.95);
    let seq_span = f64::from(high - low);
    let mut mid = low + (seq_span * fraction).round() as u32;
    if mid <= low {
        mid = low + 1;
    }
    if mid >= high {
        mid = high - 1;
    }
    mid
}

pub fn parse_timestamp(input: &str) -> Result<u64, Error> {
    let trimmed = input.trim();

    // 1. Try unix timestamp as integer
    if let Ok(num) = trimmed.parse::<u64>() {
        // If milliseconds (greater than year 2286 in seconds, ~10^10)
        if num > 10_000_000_000 {
            return Ok(num / 1000);
        }
        return Ok(num);
    }

    // 2. Try RFC3339 / ISO-8601
    if let Ok(dt) = DateTime::parse_from_rfc3339(trimmed) {
        return Ok(dt.timestamp().max(0).cast_unsigned());
    }

    // 3. Try standard UTC datetime without timezone (assume UTC)
    if let Ok(naive) = chrono::NaiveDateTime::from_str(trimmed) {
        return Ok(naive.and_utc().timestamp().max(0).cast_unsigned());
    }

    Err(Error::InvalidTimestamp(input.to_string()))
}

pub fn format_utc_seconds(sec: u64) -> String {
    Utc.timestamp_opt(sec.cast_signed(), 0)
        .single()
        .map_or_else(|| format!("{sec}"), |dt| dt.to_rfc3339())
}

pub fn find_closest_ledger_before_or_at<T>(
    ledgers: &[T],
    mut get_time: impl FnMut(&T) -> u64,
    target_sec: u64,
) -> Option<&T> {
    ledgers.iter().filter(|l| get_time(l) <= target_sec).last()
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_parse_timestamp_unix_seconds() {
        let ts = parse_timestamp("1714564800").unwrap();
        assert_eq!(ts, 1714564800);
    }

    #[test]
    fn test_parse_timestamp_unix_milliseconds() {
        let ts = parse_timestamp("1714564800000").unwrap();
        assert_eq!(ts, 1714564800);
    }

    #[test]
    fn test_parse_timestamp_rfc3339() {
        let ts = parse_timestamp("2024-05-01T12:00:00Z").unwrap();
        assert_eq!(ts, 1714564800);

        let ts2 = parse_timestamp("2024-05-01T14:00:00+02:00").unwrap();
        assert_eq!(ts2, 1714564800);
    }

    #[test]
    fn test_parse_timestamp_invalid() {
        assert!(parse_timestamp("not-a-timestamp").is_err());
    }

    #[test]
    fn test_format_utc_seconds() {
        let formatted = format_utc_seconds(1714564800);
        assert!(formatted.contains("2024-05-01"));
    }

    #[test]
    fn test_cli_clap_parsing() {
        // Test parsing --timestamp
        let cmd = Cmd::try_parse_from(["lookup", "--timestamp", "1714564800"]).unwrap();
        assert_eq!(cmd.timestamp.as_deref(), Some("1714564800"));
        assert_eq!(cmd.seq, None);

        // Test parsing --seq
        let cmd = Cmd::try_parse_from(["lookup", "--seq", "42000"]).unwrap();
        assert_eq!(cmd.seq, Some(42000));
        assert_eq!(cmd.timestamp, None);

        // Test conflict between --timestamp and --seq
        let conflict =
            Cmd::try_parse_from(["lookup", "--timestamp", "1714564800", "--seq", "42000"]);
        assert!(conflict.is_err());
    }

    #[derive(Debug, PartialEq, Eq)]
    struct MockLedger {
        sequence: u32,
        close_time: u64,
    }

    #[test]
    fn test_find_closest_ledger_before_or_at() {
        let ledgers = vec![
            MockLedger {
                sequence: 100,
                close_time: 1000,
            },
            MockLedger {
                sequence: 101,
                close_time: 1005,
            },
            MockLedger {
                sequence: 102,
                close_time: 1010,
            },
        ];

        // Exact match
        let found = find_closest_ledger_before_or_at(&ledgers, |l| l.close_time, 1005).unwrap();
        assert_eq!(found.sequence, 101);

        // In between: target = 1008 should yield ledger 101 (closed at 1005)
        let found = find_closest_ledger_before_or_at(&ledgers, |l| l.close_time, 1008).unwrap();
        assert_eq!(found.sequence, 101);

        // After all ledgers: target = 2000 should yield ledger 102 (closed at 1010)
        let found = find_closest_ledger_before_or_at(&ledgers, |l| l.close_time, 2000).unwrap();
        assert_eq!(found.sequence, 102);

        // Before all ledgers: target = 999 should yield None
        let found = find_closest_ledger_before_or_at(&ledgers, |l| l.close_time, 999);
        assert!(found.is_none());
    }

    #[test]
    fn test_lookup_result_json_serialization() {
        let res = LookupResult {
            sequence: 50000000,
            closed_at: "2024-05-01T12:00:00+00:00".to_string(),
            timestamp: 1714564800,
            hash: Some("abcd1234".to_string()),
        };
        let json = serde_json::to_string(&res).unwrap();
        assert!(json.contains("\"sequence\":50000000"));
        assert!(json.contains("\"timestamp\":1714564800"));
        assert!(json.contains("\"hash\":\"abcd1234\""));
    }
}
