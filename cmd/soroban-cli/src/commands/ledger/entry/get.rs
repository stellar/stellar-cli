
use std::fmt::Debug;

use clap::{command, Parser};
use stellar_xdr::curr::{LedgerKey, LedgerKeyAccount, MuxedAccount};
use crate::commands::config::network;
use crate::config;
use crate::config::{locator};
use crate::{
    rpc::{self},
};
use crate::commands::ledger::entry::get::Error::EmptyKeys;

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

    /// Format of the output
    #[arg(long, default_value = "original")]
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
    #[error("at least one key must be provided")]
    EmptyKeys,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, clap::ValueEnum, Default)]
pub enum OutputFormat {
    /// Original RPC output (containing XDRs)
    #[default]
    Original,
    /// JSON output of the ledger entry with parsed XDRs (one line, not formatted)
    Json,
    /// Formatted (multiline) JSON output of the ledger entry with parsed XDRs
    JsonFormatted,
}


impl Cmd {
    pub async fn run(&self) -> Result<(), Error> {
        let client = self.network.get(&self.locator)?.rpc_client()?;
        let mut ledger_keys = vec![];

        if let Some(acc) = &self.account {
            let acc = self.muxed_account(acc)?;
            let key = LedgerKey::Account(LedgerKeyAccount { account_id: acc.account_id() });
            ledger_keys.push(key);
        }

        if ledger_keys.is_empty() {
            return Err(EmptyKeys);
        }

        match self.output {
            OutputFormat::Original => {
                let resp = client.get_ledger_entries(&ledger_keys).await?;
                println!("{}", serde_json::to_string(&resp)?);
            }
            OutputFormat::Json => {
                let resp = client.get_full_ledger_entries(&ledger_keys).await?;
                println!("{}", serde_json::to_string(&resp)?);
            }
            OutputFormat::JsonFormatted => {
                let resp = client.get_full_ledger_entries(&ledger_keys).await?;
                println!("{}", serde_json::to_string_pretty(&resp)?);
            }
        }


        return Ok(());
    }

    fn muxed_account(&self, account: &str) -> Result<MuxedAccount, Error> {
        Ok(self
            .locator
            .read_identity(account)?
            .muxed_account(self.hd_path)?)
    }
}
