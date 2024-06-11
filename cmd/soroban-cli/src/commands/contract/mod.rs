pub mod asset;
pub mod bindings;
pub mod build;
pub mod deploy;
pub mod extend;
pub mod fetch;
pub mod id;
pub mod init;
pub mod inspect;
pub mod install;
pub mod invoke;
pub mod optimize;
pub mod read;
pub mod restore;

use std::{
    collections::HashMap,
    fs::{self, create_dir_all, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use stellar_strkey::DecodeError;

use crate::commands::global;

#[derive(Debug, clap::Subcommand)]
pub enum Cmd {
    /// Utilities to deploy a Stellar Asset Contract or get its id
    #[command(subcommand)]
    Asset(asset::Cmd),
    /// Generate code client bindings for a contract
    #[command(subcommand)]
    Bindings(bindings::Cmd),

    Build(build::Cmd),

    /// Extend the time to live ledger of a contract-data ledger entry.
    ///
    /// If no keys are specified the contract itself is extended.
    Extend(extend::Cmd),

    /// Deploy a wasm contract
    Deploy(deploy::wasm::Cmd),

    /// Fetch a contract's Wasm binary
    Fetch(fetch::Cmd),

    /// Generate the contract id for a given contract or asset
    #[command(subcommand)]
    Id(id::Cmd),

    /// Initialize a Soroban project with an example contract
    Init(init::Cmd),

    /// Inspect a WASM file listing contract functions, meta, etc
    Inspect(inspect::Cmd),

    /// Install a WASM file to the ledger without creating a contract instance
    Install(install::Cmd),

    /// Invoke a contract function
    ///
    /// Generates an "implicit CLI" for the specified contract on-the-fly using the contract's
    /// schema, which gets embedded into every Soroban contract. The "slop" in this command,
    /// everything after the `--`, gets passed to this implicit CLI. Get in-depth help for a given
    /// contract:
    ///
    ///     soroban contract invoke ... -- --help
    Invoke(invoke::Cmd),

    /// Optimize a WASM file
    Optimize(optimize::Cmd),

    /// Print the current value of a contract-data ledger entry
    Read(read::Cmd),

    /// Restore an evicted value for a contract-data legder entry.
    ///
    /// If no keys are specificed the contract itself is restored.
    Restore(restore::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Asset(#[from] asset::Error),

    #[error(transparent)]
    Bindings(#[from] bindings::Error),

    #[error(transparent)]
    Build(#[from] build::Error),

    #[error(transparent)]
    Extend(#[from] extend::Error),

    #[error(transparent)]
    Deploy(#[from] deploy::wasm::Error),

    #[error(transparent)]
    Fetch(#[from] fetch::Error),

    #[error(transparent)]
    Init(#[from] init::Error),

    #[error(transparent)]
    Id(#[from] id::Error),

    #[error(transparent)]
    Inspect(#[from] inspect::Error),

    #[error(transparent)]
    Install(#[from] install::Error),

    #[error(transparent)]
    Invoke(#[from] invoke::Error),

    #[error(transparent)]
    Optimize(#[from] optimize::Error),

    #[error(transparent)]
    Read(#[from] read::Error),

    #[error(transparent)]
    Restore(#[from] restore::Error),
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        match &self {
            Cmd::Asset(asset) => asset.run().await?,
            Cmd::Bindings(bindings) => bindings.run().await?,
            Cmd::Build(build) => build.run()?,
            Cmd::Extend(extend) => extend.run().await?,
            Cmd::Deploy(deploy) => deploy.run().await?,
            Cmd::Id(id) => id.run()?,
            Cmd::Init(init) => init.run()?,
            Cmd::Inspect(inspect) => inspect.run()?,
            Cmd::Install(install) => install.run().await?,
            Cmd::Invoke(invoke) => invoke.run(global_args).await?,
            Cmd::Optimize(optimize) => optimize.run()?,
            Cmd::Fetch(fetch) => fetch.run().await?,
            Cmd::Read(read) => read.run().await?,
            Cmd::Restore(restore) => restore.run().await?,
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, clap::ValueEnum)]
pub enum Durability {
    /// Persistent
    Persistent,
    /// Temporary
    Temporary,
}

impl From<&Durability> for soroban_env_host::xdr::ContractDataDurability {
    fn from(d: &Durability) -> Self {
        match d {
            Durability::Persistent => soroban_env_host::xdr::ContractDataDurability::Persistent,
            Durability::Temporary => soroban_env_host::xdr::ContractDataDurability::Temporary,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, clap::ValueEnum)]
pub enum SpecOutput {
    /// XDR of array of contract spec entries
    XdrBase64,
    /// Array of xdr of contract spec entries
    XdrBase64Array,
    /// Pretty print of contract spec entries
    Docs,
}

#[derive(Serialize, Deserialize, Default)]
pub struct AliasData {
    ids: HashMap<String, String>,
}

#[derive(thiserror::Error, Debug)]
pub enum AliasDataError {
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("cannot access config dir for alias file")]
    CannotAccessConfigDir,
    #[error("cannot parse contract ID {0}: {1}")]
    CannotParseContractId(String, DecodeError),
}

impl AliasData {
    pub fn load(config_dir: &Path, alias: Option<&str>) -> Result<Option<Self>, AliasDataError> {
        let Some(alias) = alias else {
            return Ok(None);
        };

        let path = Self::alias_path(config_dir, alias);

        if !path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(path)?;
        let data: Self = serde_json::from_str(&content).unwrap_or_default();

        Ok(Some(data))
    }

    pub fn alias_path(config_dir: &Path, alias: &str) -> PathBuf {
        let file_name = format!("{alias}.json");
        config_dir.join("contract-ids").join(file_name)
    }

    pub fn save_contract_id(
        config_dir: &Path,
        contract_id: &str,
        alias: Option<&String>,
        network_passphrase: &str,
    ) -> Result<(), AliasDataError> {
        let Some(alias) = alias else {
            return Ok(());
        };

        let path = Self::alias_path(config_dir, alias);
        let dir = path.parent().ok_or(AliasDataError::CannotAccessConfigDir)?;

        create_dir_all(dir).map_err(|_| AliasDataError::CannotAccessConfigDir)?;

        let content = fs::read_to_string(&path).unwrap_or_default();
        let mut data: Self = serde_json::from_str(&content).unwrap_or_default();

        let mut to_file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(path)?;

        data.ids
            .insert(network_passphrase.into(), contract_id.into());

        let content = serde_json::to_string(&data)?;

        Ok(to_file.write_all(content.as_bytes())?)
    }

    pub fn get_contract_id(
        alias: &str,
        config_dir: &Path,
        network_passphrase: &str,
    ) -> Result<Option<String>, AliasDataError> {
        let alias_data = Self::load(config_dir, Some(alias))?;
        let Some(alias_data) = alias_data else {
            return Ok(None);
        };

        match alias_data.ids.get(network_passphrase) {
            Some(id) => Ok(Some(id.into())),
            _ => Ok(None),
        }
    }

    pub fn load_contract_id_or_default(
        alias: &str,
        config_dir: &Path,
        network_passphrase: &str,
    ) -> Result<[u8; 32], AliasDataError> {
        let contract_id = match Self::get_contract_id(alias, config_dir, network_passphrase)? {
            None => alias.to_string(),
            Some(id) => id,
        };

        soroban_spec_tools::utils::contract_id_from_str(&contract_id)
            .map_err(|e| AliasDataError::CannotParseContractId(contract_id.clone(), e))
    }
}
