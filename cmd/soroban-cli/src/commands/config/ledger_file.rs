use crate::{commands::HEADING_SANDBOX, utils};
use clap::arg;
use soroban_ledger_snapshot::LedgerSnapshot;
use std::path::PathBuf;

#[derive(Debug, clap::Args, Clone)]
#[group(skip)]
pub struct Args {
    /// File to persist ledger state
    #[arg(
        long,
        default_value(".soroban/ledger.json"),
        env = "SOROBAN_LEDGER_FILE",
        help_heading = HEADING_SANDBOX,
    )]
    pub ledger_file: PathBuf,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            ledger_file: ".soroban/ledger.json".into(),
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("reading file {filepath}: {error}")]
    CannotReadLedgerFile {
        filepath: PathBuf,
        error: soroban_ledger_snapshot::Error,
    },

    #[error("committing file {filepath}: {error}")]
    CannotCommitLedgerFile {
        filepath: PathBuf,
        error: soroban_ledger_snapshot::Error,
    },
}

impl Args {
    pub fn read(&self) -> Result<LedgerSnapshot, Error> {
        utils::ledger_snapshot_read_or_default(&self.ledger_file).map_err(|e| {
            Error::CannotReadLedgerFile {
                filepath: self.ledger_file.clone(),
                error: e,
            }
        })
    }

    pub fn write(&self, state: &mut LedgerSnapshot) -> Result<(), Error> {
        state
            .write_file(&self.ledger_file)
            .map_err(|e| Error::CannotCommitLedgerFile {
                filepath: self.ledger_file.clone(),
                error: e,
            })
    }
}
