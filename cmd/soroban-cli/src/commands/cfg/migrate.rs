use super::super::config::locator;
use crate::commands::cfg::migrate::Error::InvalidFile;
use crate::config::locator::{KeyType, Location};
use crate::print::Print;
use clap::command;
use sha2::{Digest, Sha256};
use std::fs::create_dir_all;
use std::path::{Path, PathBuf};
use std::{fs, io};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Config(#[from] locator::Error),
    #[error(transparent)]
    StripPrefix(#[from] std::path::StripPrefixError),
    #[error("Unexpected invalid file: {0}")]
    InvalidFile(PathBuf),
    #[error(transparent)]
    Io(#[from] io::Error),
}

#[derive(Debug, clap::Parser, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub locator: locator::Args,
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        let print = Print::new(false);

        let identities = self.local_configs_path(KeyType::Identity)?;
        let networks = self.local_configs_path(KeyType::Network)?;
        let contract_ids = self.local_configs_path(KeyType::ContractIds)?;

        if identities.is_empty() && networks.is_empty() && contract_ids.is_empty() {
            print.checkln("Config is already fully migrated");
            return Ok(());
        }

        self.migrate(identities, "identity")?;
        self.migrate(networks, "network")?;
        self.migrate(contract_ids, "contract alias")?;

        Self::try_delete(self.locator.local_config()?, "local")?;

        Ok(())
    }

    fn local_configs_path(&self, key_type: KeyType) -> Result<Vec<PathBuf>, Error> {
        Ok(key_type
            .list_paths_silent(&self.locator.local_and_global()?)?
            .into_iter()
            .filter_map(|(_, location)| match location {
                Location::Local(path) => Some(path),
                Location::Global(_) => None,
            })
            .collect::<Vec<_>>())
    }

    fn migrate<P: AsRef<Path>>(&self, locations: Vec<P>, config_type: &str) -> Result<(), Error> {
        let print = Print::new(false);
        let mut local = None;

        for location in locations {
            let path = location.as_ref();
            let destination_root = self.locator.config_dir()?;
            let destination_suffix = path.strip_prefix(self.locator.local_config()?)?;
            let mut target = destination_root.join(destination_suffix);
            if target.exists() {
                let extension = target.extension().ok_or(InvalidFile(target.clone()))?;
                let original_name = target
                    .file_stem()
                    .ok_or(InvalidFile(target.clone()))?
                    .to_str()
                    .ok_or(InvalidFile(target.clone()))?;
                let sha256 = Sha256::digest(path.display().to_string().as_bytes());
                let sha256 = format!("{sha256:x}").chars().take(8).collect::<String>();
                let name = format!("migrated_{original_name}_{sha256}");
                print.warnln(format!("Duplicated '{original_name}' {config_type} found: it will be renamed to {name}"));
                target = target.with_file_name(&name).with_extension(extension);
            }
            create_dir_all(target.parent().unwrap())?;
            fs::copy(path, &target)?;
            fs::remove_file(path)?;
            print.infoln(format!(
                "Moved {} from {} to {}",
                config_type,
                path.display(),
                target.display()
            ));
            local = Some(location);
        }

        if let Some(location) = local {
            let parent = location.as_ref().parent().unwrap();
            Self::try_delete(parent, config_type)?;
        }

        Ok(())
    }

    fn try_delete<P: AsRef<Path>>(path: P, config_type: &str) -> Result<(), Error> {
        let print = Print::new(false);
        let path = path.as_ref();

        let is_empty = path.read_dir()?.next().is_none();
        if is_empty {
            print.infoln(format!(
                "Deleted fully migrated {} config directory {}",
                config_type,
                path.display()
            ));
            fs::remove_dir(path)?;
        } else {
            print.warnln(format!(
                "Couldn't delete {} because it's not empty",
                path.display()
            ));
        }

        Ok(())
    }
}
