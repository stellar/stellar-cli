use clap::Parser;
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
                Location::Local(config_dir) => {
                    if config_dir.exists() {
                        print_deprecation_warning(&config_dir);
                    }
                }
                Location::Global(config_dir) => Self::read_from_config_dir(&config_dir)?,
            }
        }

        Ok(())
    }

    fn collect_aliases(config_dir: &Path) -> Result<HashMap<String, Vec<AliasEntry>>, Error> {
        let dir = glob::Pattern::escape(&config_dir.join("contract-ids").to_string_lossy());
        let pattern = Path::new(&dir)
            .join("*.json")
            .to_string_lossy()
            .into_owned();

        let paths = glob::glob(&pattern)?;
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

                    map.entry(network_passphrase.clone())
                        .or_default()
                        .push(entry);
                }
            }
        }

        Ok(map)
    }

    fn read_from_config_dir(config_dir: &Path) -> Result<(), Error> {
        let map = Self::collect_aliases(config_dir)?;
        let mut found = false;

        for network_passphrase in map.keys() {
            if let Some(list) = map.clone().get_mut(network_passphrase) {
                println!("ℹ️ Aliases available for network '{network_passphrase}'");

                list.sort_by(|a, b| a.alias.cmp(&b.alias));

                for entry in list {
                    found = true;
                    println!("{}: {}", entry.alias, entry.contract);
                }

                println!();
            }
        }

        if !found {
            eprintln!("⚠️ No aliases defined for network");

            process::exit(1);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_alias(dir: &Path, name: &str, network: &str, contract: &str) {
        let contract_ids_dir = dir.join("contract-ids");
        fs::create_dir_all(&contract_ids_dir).unwrap();
        let content = format!(r#"{{"ids":{{"{network}":"{contract}"}}}}"#);
        fs::write(contract_ids_dir.join(format!("{name}.json")), content).unwrap();
    }

    #[test]
    fn glob_metacharacters_in_config_dir_are_treated_as_literal() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();

        // Sibling directories that would match the glob `[12]` if unescaped.
        write_alias(&base.join("cfg1"), "alpha", "testnet", "CAAAA");
        write_alias(&base.join("cfg2"), "beta", "testnet", "CBBBB");

        // The literal directory whose name contains bracket metacharacters.
        write_alias(&base.join("cfg[12]"), "gamma", "testnet", "CCCCC");

        let map = Cmd::collect_aliases(&base.join("cfg[12]")).unwrap();

        let aliases: Vec<&str> = map
            .values()
            .flat_map(|entries| entries.iter().map(|e| e.alias.as_str()))
            .collect();

        assert!(
            aliases.contains(&"gamma"),
            "should read alias from the literal directory"
        );
        assert!(
            !aliases.contains(&"alpha"),
            "should not read from sibling cfg1"
        );
        assert!(
            !aliases.contains(&"beta"),
            "should not read from sibling cfg2"
        );
    }
}
