use clap::Parser;
use soroban_ledger_snapshot::LedgerSnapshot;
use std::{collections::HashMap, path::PathBuf};
use stellar_xdr::curr::LedgerKey;

use crate::{commands::global, print};

fn default_out_path() -> PathBuf {
    PathBuf::new().join("snapshot.json")
}

/// Merge multiple ledger snapshots into a single snapshot file.
///
/// When the same ledger key appears in multiple snapshots, the entry from
/// the last snapshot in the argument list takes precedence. Metadata
/// (protocol_version, sequence_number, timestamp, etc.) is taken from the
/// last snapshot.
///
/// Example:
///   stellar snapshot merge A.json B.json --out merged.json
///
/// This allows combining snapshots from different contract deployments or
/// manually edited snapshots without regenerating from scratch.
#[derive(Parser, Debug, Clone)]
#[command(arg_required_else_help = true)]
pub struct Cmd {
    /// Snapshot files to merge (at least 2 required)
    #[arg(required = true, num_args = 2..)]
    snapshots: Vec<PathBuf>,

    /// Output path for the merged snapshot
    #[arg(long, short, default_value=default_out_path().into_os_string())]
    out: PathBuf,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("failed to read snapshot file '{path}': {error}")]
    ReadSnapshot {
        path: PathBuf,
        error: soroban_ledger_snapshot::Error,
    },

    #[error("failed to write merged snapshot to '{path}': {error}")]
    WriteSnapshot {
        path: PathBuf,
        error: soroban_ledger_snapshot::Error,
    },

    #[error("at least 2 snapshot files are required for merging")]
    InsufficientSnapshots,
}

impl Cmd {
    pub fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        if self.snapshots.len() < 2 {
            return Err(Error::InsufficientSnapshots);
        }

        let print = print::Print::new(global_args.quiet);

        // Read all snapshots
        let mut snapshots = Vec::new();
        for path in &self.snapshots {
            let snapshot =
                LedgerSnapshot::read_file(path).map_err(|error| Error::ReadSnapshot {
                    path: path.clone(),
                    error,
                })?;
            snapshots.push(snapshot);
        }

        // Merge snapshots
        let merged = self.merge_snapshots(snapshots);

        // Write merged snapshot
        merged
            .write_file(&self.out)
            .map_err(|error| Error::WriteSnapshot {
                path: self.out.clone(),
                error,
            })?;

        print.checkln(format!(
            "Merged snapshot written to: {}",
            self.out.display()
        ));

        Ok(())
    }

    fn merge_snapshots(&self, snapshots: Vec<LedgerSnapshot>) -> LedgerSnapshot {
        // Use a HashMap to track entries by key, with last-wins semantics
        let mut merged_entries: HashMap<
            LedgerKey,
            (Box<stellar_xdr::curr::LedgerEntry>, Option<u32>),
        > = HashMap::new();

        // Iterate through snapshots in order, so later entries override earlier ones
        for snapshot in &snapshots {
            for (key, (entry, ttl)) in &snapshot.ledger_entries {
                merged_entries.insert((**key).clone(), (entry.clone(), *ttl));
            }
        }

        // Take metadata from the last snapshot
        let last_snapshot = snapshots.last().unwrap(); // Safe because we checked len >= 2

        // Build the final merged snapshot
        LedgerSnapshot {
            protocol_version: last_snapshot.protocol_version,
            sequence_number: last_snapshot.sequence_number,
            timestamp: last_snapshot.timestamp,
            network_id: last_snapshot.network_id,
            base_reserve: last_snapshot.base_reserve,
            min_persistent_entry_ttl: last_snapshot.min_persistent_entry_ttl,
            min_temp_entry_ttl: last_snapshot.min_temp_entry_ttl,
            max_entry_ttl: last_snapshot.max_entry_ttl,
            ledger_entries: merged_entries
                .into_iter()
                .map(|(k, (e, ttl))| (Box::new(k), (e, ttl)))
                .collect(),
        }
    }
}
