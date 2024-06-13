use std::{
    collections::HashMap,
    fs::{self, create_dir_all, OpenOptions},
    io::Write,
    path::PathBuf,
};

use serde::{Deserialize, Serialize};
use stellar_strkey::DecodeError;

use crate::commands::config;

use super::Args;

#[derive(Serialize, Deserialize, Default)]
pub struct Data {
    ids: HashMap<String, String>,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("cannot access config dir for alias file")]
    CannotAccessConfigDir,
    #[error("cannot parse contract ID {0}: {1}")]
    CannotParseContractId(String, DecodeError),
    #[error(transparent)]
    Config(#[from] config::Error),
}

impl Args {
    pub fn load(&self, alias: &str) -> Result<Option<Data>, Error> {
        let path = self.alias_path(alias)?;

        if !path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(path)?;
        let data: Data = serde_json::from_str(&content).unwrap_or_default();

        Ok(Some(data))
    }

    fn alias_path(&self, alias: &str) -> Result<PathBuf, Error> {
        let file_name = format!("{alias}.json");
        let config_dir = self.config_dir()?;
        Ok(config_dir.join("contract-ids").join(file_name))
    }

    pub fn save_contract_id(&self, contract_id: &str, alias: Option<&String>) -> Result<(), Error> {
        let Some(alias) = alias else {
            return Ok(());
        };

        let path = self.alias_path(alias)?;
        let dir = path.parent().ok_or(Error::CannotAccessConfigDir)?;

        create_dir_all(dir).map_err(|_| Error::CannotAccessConfigDir)?;

        let content = fs::read_to_string(&path).unwrap_or_default();
        let mut data: Data = serde_json::from_str(&content).unwrap_or_default();

        let mut to_file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(path)?;

        let network = self.get_network()?;

        data.ids
            .insert(network.network_passphrase, contract_id.into());

        let content = serde_json::to_string(&data)?;

        Ok(to_file.write_all(content.as_bytes())?)
    }

    pub fn get_contract_id(
        &self,
        alias: &str,
        network_passphrase: &str,
    ) -> Result<Option<String>, Error> {
        let Some(alias_data) = self.load(alias)? else {
            return Ok(None);
        };

        Ok(alias_data.ids.get(network_passphrase).cloned())
    }

    pub fn resolve_contract_id(
        &self,
        alias_or_contract_id: &str,
        network_passphrase: &str,
    ) -> Result<[u8; 32], Error> {
        let contract_id = self
            .get_contract_id(alias_or_contract_id, network_passphrase)?
            .unwrap_or_else(|| alias_or_contract_id.to_string());

        soroban_spec_tools::utils::contract_id_from_str(&contract_id)
            .map_err(|e| Error::CannotParseContractId(contract_id.clone(), e))
    }
}
