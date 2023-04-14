use crate::{commands::HEADING_SANDBOX, utils};
use clap::arg;
use soroban_ledger_snapshot::LedgerSnapshot;
use std::path::{Path, PathBuf};

#[derive(Debug, clap::Args, Clone, Default)]
#[group(skip)]
pub struct Args {
    /// File to persist ledger state, default is `.soroban/ledger.json`
    #[arg(
        long,
        env = "SOROBAN_LEDGER_FILE",
        help_heading = HEADING_SANDBOX,
    )]
    pub ledger_file: Option<PathBuf>,
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
    pub fn read(&self, pwd: &Path) -> Result<LedgerSnapshot, Error> {
        let filepath = self.path(pwd);
        utils::ledger_snapshot_read_or_default(&filepath)
            .map_err(|e| Error::CannotReadLedgerFile { filepath, error: e })
    }

    pub fn write(&self, state: &mut LedgerSnapshot, pwd: &Path) -> Result<(), Error> {
        let filepath = self.path(pwd);

        state
            .write_file(&filepath)
            .map_err(|e| Error::CannotCommitLedgerFile { filepath, error: e })
    }

    pub fn path(&self, pwd: &Path) -> PathBuf {
        if let Some(path) = &self.ledger_file {
            path.clone()
        } else {
            pwd.join("ledger.json")
        }
    }
}
