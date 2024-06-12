use std::{
    collections::HashMap,
    fs::{self, create_dir_all, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use stellar_strkey::DecodeError;

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
}

impl Data {
    pub fn load(config_dir: &Path, alias: Option<&str>) -> Result<Option<Self>, Error> {
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
    ) -> Result<(), Error> {
        let Some(alias) = alias else {
            return Ok(());
        };

        let path = Self::alias_path(config_dir, alias);
        let dir = path.parent().ok_or(Error::CannotAccessConfigDir)?;

        create_dir_all(dir).map_err(|_| Error::CannotAccessConfigDir)?;

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
    ) -> Result<Option<String>, Error> {
        let Some(alias_data) = Self::load(config_dir, Some(alias))? else {
            return Ok(None);
        };

        Ok(alias_data.ids.get(network_passphrase).cloned())
    }

    pub fn load_contract_id_or_default(
        alias_or_contract_id: &str,
        config_dir: &Path,
        network_passphrase: &str,
    ) -> Result<[u8; 32], Error> {
        let contract_id =
            Self::get_contract_id(alias_or_contract_id, config_dir, network_passphrase)?
                .unwrap_or_else(|| alias_or_contract_id.to_string());

        soroban_spec_tools::utils::contract_id_from_str(&contract_id)
            .map_err(|e| Error::CannotParseContractId(contract_id.clone(), e))
    }
}
