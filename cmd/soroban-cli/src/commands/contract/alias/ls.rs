use clap::{command, Parser};
use std::collections::HashMap;
use std::fmt::Debug;
use std::path::Path;
use std::{fs, process};

use crate::commands::config::network;
use crate::config::locator::{print_deprecation_warning, Location};
use crate::config::{alias, locator};

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub config_locator: locator::Args,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Locator(#[from] locator::Error),

    #[error(transparent)]
    Network(#[from] network::Error),

    #[error(transparent)]
    PatternError(#[from] glob::PatternError),

    #[error(transparent)]
    GlobError(#[from] glob::GlobError),

    #[error(transparent)]
    IoError(#[from] std::io::Error),
}

#[derive(Debug, Clone)]
struct AliasEntry {
    alias: String,
    contract: String,
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        let config_dirs = self.config_locator.local_and_global()?;

        for cfg in config_dirs {
            match cfg {
                Location::Local(config_dir) => Self::read_from_config_dir(&config_dir, true)?,
                Location::Global(config_dir) => Self::read_from_config_dir(&config_dir, false)?,
            }
        }

        Ok(())
    }

    fn read_from_config_dir(config_dir: &Path, deprecation_mode: bool) -> Result<(), Error> {
        let pattern = config_dir
            .join("contract-ids")
            .join("*.json")
            .to_string_lossy()
            .into_owned();

        let paths = glob::glob(&pattern)?;
        let mut found = false;
        let mut map: HashMap<String, Vec<AliasEntry>> = HashMap::new();

        for path in paths {
            let path = path?;

            if let Some(alias) = path.file_stem() {
                let alias = alias.to_string_lossy().into_owned();
                let content = fs::read_to_string(path)?;
                let data: alias::Data = serde_json::from_str(&content).unwrap_or_default();

                for network_passphrase in data.ids.keys() {
                    let network_passphrase = network_passphrase.clone();
                    let contract = data
                        .ids
                        .get(&network_passphrase)
                        .map(ToString::to_string)
                        .unwrap_or_default();
                    let entry = AliasEntry {
                        alias: alias.clone(),
                        contract,
                    };

                    let list = map.entry(network_passphrase.clone()).or_default();

                    list.push(entry.clone());
                }
            }
        }

        for network_passphrase in map.keys() {
            if let Some(list) = map.clone().get_mut(network_passphrase) {
                println!("ℹ️ Aliases available for network '{network_passphrase}'");

                list.sort_by(|a, b| a.alias.cmp(&b.alias));

                for entry in list {
                    if !found && deprecation_mode {
                        print_deprecation_warning(config_dir);
                    }
                    found = true;
                    println!("{}: {}", entry.alias, entry.contract);
                }

                println!();
            }
        }

        if !found && !deprecation_mode {
            eprintln!("⚠️ No aliases defined for network");

            process::exit(1);
        }

        Ok(())
    }
}
