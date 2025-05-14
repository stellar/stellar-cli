use crate::{
    commands::config::{locator, network},
    rpc,
    xdr::{
        ClaimableBalanceId::ClaimableBalanceIdTypeV0, Hash, LedgerKey, LedgerKeyClaimableBalance,
    },
};
use clap::{command, Parser};
use hex::FromHexError;
use soroban_spec_tools::utils::padded_hex_from_str;
use super::OutputFormat;

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub network: network::Args,

    #[command(flatten)]
    pub locator: locator::Args,

    /// Claimable Balance Ids to fetch an entry for
    pub ids: Vec<String>,

    /// Format of the output
    #[arg(long, default_value = "json")]
    pub output: OutputFormat,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    FromHexError(#[from] FromHexError),
    #[error("provided hash value is invalid: {0}")]
    InvalidHash(String),
    #[error(transparent)]
    Network(#[from] network::Error),
    #[error(transparent)]
    Rpc(#[from] rpc::Error),
    #[error(transparent)]
    Serde(#[from] serde_json::Error),
}

impl Cmd {
    pub async fn run(&self) -> Result<(), Error> {
        let network = self.network.get(&self.locator)?;
        let client = network.rpc_client()?;
        let mut ledger_keys = vec![];

        self.insert_keys(&mut ledger_keys)?;

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

    fn insert_keys(&self, ledger_keys: &mut Vec<LedgerKey>) -> Result<(), Error> {
        for x in &self.ids {
            let padded_hex = padded_hex_from_str(x, 32)?;
            let hash_bytes: [u8; 32] = padded_hex
                .try_into()
                .map_err(|_| Error::InvalidHash(x.to_string()))?;
            let hash = Hash(hash_bytes);
            let key = LedgerKey::ClaimableBalance(LedgerKeyClaimableBalance {
                balance_id: ClaimableBalanceIdTypeV0(hash),
            });
            ledger_keys.push(key);
        }
        Ok(())
    }
}
