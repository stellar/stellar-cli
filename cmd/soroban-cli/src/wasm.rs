use clap::arg;
use sha2::{Digest, Sha256};
use soroban_env_host::xdr::{self, Hash, LedgerKey, LedgerKeyContractCode};
use soroban_spec_tools::contract::{self, Spec};
use std::{
    fs, io,
    path::{Path, PathBuf},
};
use stellar_xdr::curr::{ContractDataEntry, ContractExecutable, ScVal};

use crate::{
    config::{locator, network::Network},
    rpc_client::{Error as RpcClientError, RpcClient},
    utils::{self, rpc::get_remote_wasm_from_hash},
    wasm::Error::{ContractIsStellarAsset, UnexpectedContractToken},
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("reading file {filepath}: {error}")]
    CannotReadContractFile {
        filepath: std::path::PathBuf,
        error: io::Error,
    },
    #[error("cannot parse wasm file {file}: {error}")]
    CannotParseWasm {
        file: std::path::PathBuf,
        error: wasmparser::BinaryReaderError,
    },
    #[error("xdr processing error: {0}")]
    Xdr(#[from] xdr::Error),

    #[error(transparent)]
    Parser(#[from] wasmparser::BinaryReaderError),
    #[error(transparent)]
    ContractSpec(#[from] contract::Error),

    #[error(transparent)]
    Locator(#[from] locator::Error),
    #[error(transparent)]
    Rpc(#[from] soroban_rpc::Error),
    #[error("unexpected contract data {0:?}")]
    UnexpectedContractToken(ContractDataEntry),
    #[error(
        "cannot fetch wasm for contract because the contract is \
    a network built-in asset contract that does not have a downloadable code binary"
    )]
    ContractIsStellarAsset,
    #[error(transparent)]
    RpcClient(#[from] RpcClientError),
}

#[derive(Debug, clap::Args, Clone)]
#[group(skip)]
pub struct Args {
    /// Path to wasm binary
    #[arg(long)]
    pub wasm: PathBuf,
}

impl Args {
    /// # Errors
    /// May fail to read wasm file
    pub fn read(&self) -> Result<Vec<u8>, Error> {
        fs::read(&self.wasm).map_err(|e| Error::CannotReadContractFile {
            filepath: self.wasm.clone(),
            error: e,
        })
    }

    /// # Errors
    /// May fail to read wasm file
    pub fn len(&self) -> Result<u64, Error> {
        len(&self.wasm)
    }

    /// # Errors
    /// May fail to read wasm file
    pub fn is_empty(&self) -> Result<bool, Error> {
        self.len().map(|len| len == 0)
    }

    /// # Errors
    /// May fail to read wasm file or parse xdr section
    pub fn parse(&self) -> Result<Spec, Error> {
        let contents = self.read()?;
        Ok(Spec::new(&contents)?)
    }

    pub fn hash(&self) -> Result<Hash, Error> {
        Ok(Hash(Sha256::digest(self.read()?).into()))
    }
}

impl From<&PathBuf> for Args {
    fn from(wasm: &PathBuf) -> Self {
        Self { wasm: wasm.clone() }
    }
}

impl TryInto<LedgerKey> for Args {
    type Error = Error;
    fn try_into(self) -> Result<LedgerKey, Self::Error> {
        Ok(LedgerKey::ContractCode(LedgerKeyContractCode {
            hash: utils::contract_hash(&self.read()?)?,
        }))
    }
}

/// # Errors
/// May fail to read wasm file
pub fn len(p: &Path) -> Result<u64, Error> {
    Ok(std::fs::metadata(p)
        .map_err(|e| Error::CannotReadContractFile {
            filepath: p.to_path_buf(),
            error: e,
        })?
        .len())
}

pub async fn fetch_from_contract(
    contract_id: &str,
    network: &Network,
    locator: &locator::Args,
) -> Result<Vec<u8>, Error> {
    tracing::trace!(?network);

    let contract_id = &locator
        .resolve_contract_id(contract_id, &network.network_passphrase)?
        .0;

    let client = RpcClient::new(network.clone())?;
    client
        .verify_network_passphrase(Some(&network.network_passphrase))
        .await?;
    let data_entry = client.get_contract_data(contract_id).await?;
    if let ScVal::ContractInstance(contract) = &data_entry.val {
        return match &contract.executable {
            ContractExecutable::Wasm(hash) => Ok(get_remote_wasm_from_hash(&client, hash).await?),
            ContractExecutable::StellarAsset => Err(ContractIsStellarAsset),
        };
    }
    Err(UnexpectedContractToken(data_entry))
}
