use crate::commands::config::network;
use crate::config;
use crate::config::locator;
use crate::rpc;
use clap::{command, Parser};
use stellar_xdr::curr::{Hash, LedgerKey, LedgerKeyContractCode};

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub network: network::Args,

    #[command(flatten)]
    pub locator: locator::Args,

    /// Get WASM bytecode by hash
    pub wasm_hashes: Vec<String>,

    /// Format of the output
    #[arg(long, default_value = "json")]
    pub output: OutputFormat,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Config(#[from] config::key::Error),
    #[error(transparent)]
    Locator(#[from] locator::Error),
    #[error(transparent)]
    Network(#[from] network::Error),
    #[error(transparent)]
    Rpc(#[from] rpc::Error),
    #[error(transparent)]
    Serde(#[from] serde_json::Error),
    #[error(transparent)]
    Spec(#[from] soroban_spec_tools::Error),
    #[error(transparent)]
    StellarXdr(#[from] stellar_xdr::curr::Error),
    #[error("provided hash value is invalid: {0}")]
    InvalidHash(String),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, clap::ValueEnum, Default)]
pub enum OutputFormat {
    /// JSON output of the ledger entry with parsed XDRs (one line, not formatted)
    #[default]
    Json,
    /// Formatted (multiline) JSON output of the ledger entry with parsed XDRs
    JsonFormatted,
    /// Original RPC output (containing XDRs)
    Xdr,
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
        for wasm_hash in &self.wasm_hashes {
            let hash = Hash(
                soroban_spec_tools::utils::contract_id_from_str(wasm_hash)
                    .map_err(|_| Error::InvalidHash(wasm_hash.clone()))?,
            );
            let key = LedgerKey::ContractCode(LedgerKeyContractCode { hash });
            ledger_keys.push(key);
        }

        Ok(())
    }
}
