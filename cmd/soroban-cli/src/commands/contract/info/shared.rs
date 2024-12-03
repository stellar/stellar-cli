use std::path::PathBuf;

use clap::arg;

use crate::{
    commands::contract::info::shared::Error::InvalidWasmHash,
    config::{self, locator, network},
    utils::rpc::get_remote_wasm_from_hash,
    wasm::{self, Error::ContractIsStellarAsset},
    xdr,
};

#[derive(Debug, clap::Args, Clone, Default)]
#[command(group(
    clap::ArgGroup::new("Source")
    .required(true)
    .args(& ["wasm", "wasm_hash", "contract_id"]),
))]
#[group(skip)]
pub struct Args {
    /// Wasm file to extract the data from
    #[arg(long, group = "Source")]
    pub wasm: Option<PathBuf>,
    /// Wasm hash to get the data for
    #[arg(long = "wasm-hash", group = "Source")]
    pub wasm_hash: Option<String>,
    /// Contract id or contract alias to get the data for
    #[arg(long = "id", env = "STELLAR_CONTRACT_ID", group = "Source")]
    pub contract_id: Option<config::UnresolvedContract>,
    #[command(flatten)]
    pub network: network::Args,
    #[command(flatten)]
    pub locator: locator::Args,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, clap::ValueEnum, Default)]
pub enum MetasInfoOutput {
    /// Text output of the meta info entry
    #[default]
    Text,
    /// XDR output of the info entry
    XdrBase64,
    /// JSON output of the info entry (one line, not formatted)
    Json,
    /// Formatted (multiline) JSON output of the info entry
    JsonFormatted,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Network(#[from] network::Error),
    #[error(transparent)]
    Wasm(#[from] wasm::Error),
    #[error("provided wasm hash is invalid {0:?}")]
    InvalidWasmHash(String),
    #[error(transparent)]
    Rpc(#[from] soroban_rpc::Error),
    #[error(transparent)]
    Locator(#[from] locator::Error),
}

pub async fn fetch_wasm(args: &Args) -> Result<Option<Vec<u8>>, Error> {
    // Check if a local WASM file path is provided
    if let Some(path) = &args.wasm {
        // Read the WASM file and return its contents
        let wasm_bytes = wasm::Args { wasm: path.clone() }.read()?;
        return Ok(Some(wasm_bytes));
    }

    // If no local wasm, then check for wasm_hash and fetch from the network
    let network = &args.network.get(&args.locator)?;
    let wasm = if let Some(wasm_hash) = &args.wasm_hash {
        let hash = hex::decode(wasm_hash)
            .map_err(|_| InvalidWasmHash(wasm_hash.clone()))?
            .try_into()
            .map_err(|_| InvalidWasmHash(wasm_hash.clone()))?;

        let hash = xdr::Hash(hash);

        let client = network.rpc_client()?;

        client
            .verify_network_passphrase(Some(&network.network_passphrase))
            .await?;

        get_remote_wasm_from_hash(&client, &hash).await?
    } else if let Some(contract_id) = &args.contract_id {
        let contract_id =
            contract_id.resolve_contract_id(&args.locator, &network.network_passphrase)?;
        let res = wasm::fetch_from_contract(&contract_id, network).await;
        if let Some(ContractIsStellarAsset) = res.as_ref().err() {
            return Ok(None);
        }
        res?
    } else {
        unreachable!("One of contract location arguments must be passed");
    };

    Ok(Some(wasm))
}
