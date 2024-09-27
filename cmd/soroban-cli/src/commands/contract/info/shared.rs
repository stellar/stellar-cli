use std::path::PathBuf;

use clap::arg;
use soroban_env_host::xdr;

use crate::{
    commands::contract::info::shared::Error::InvalidWasmHash,
    config::{locator, network},
    rpc_client::{Error as RpcClientError, RpcClient},
    utils::rpc::get_remote_wasm_from_hash,
    wasm::{self, Error::ContractIsStellarAsset},
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
    /// Contract id to get the data for
    #[arg(long = "id", env = "STELLAR_CONTRACT_ID", group = "Source")]
    pub contract_id: Option<String>,
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
    RpcClient(#[from] RpcClientError),
}

pub async fn fetch_wasm(args: &Args) -> Result<Option<Vec<u8>>, Error> {
    let network = &args.network.get(&args.locator)?;

    let wasm = if let Some(path) = &args.wasm {
        wasm::Args { wasm: path.clone() }.read()?
    } else if let Some(wasm_hash) = &args.wasm_hash {
        let hash = hex::decode(wasm_hash)
            .map_err(|_| InvalidWasmHash(wasm_hash.clone()))?
            .try_into()
            .map_err(|_| InvalidWasmHash(wasm_hash.clone()))?;

        let hash = xdr::Hash(hash);

        let client = RpcClient::new(network.clone())?;

        client
            .verify_network_passphrase(Some(&network.network_passphrase))
            .await?;

        get_remote_wasm_from_hash(&client, &hash).await?
    } else if let Some(contract_id) = &args.contract_id {
        let res = wasm::fetch_from_contract(contract_id, network, &args.locator).await;
        if let Some(ContractIsStellarAsset) = res.as_ref().err() {
            return Ok(None);
        }
        res?
    } else {
        unreachable!("One of contract location arguments must be passed");
    };

    Ok(Some(wasm))
}
