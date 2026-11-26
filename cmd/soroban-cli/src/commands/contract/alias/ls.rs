use clap::{Parser, ValueEnum};
use serde::Serialize;
use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fmt::Debug;
use std::fs;
use std::path::Path;

use crate::commands::config::network;
use crate::commands::global;
use crate::config::locator::{print_deprecation_warning, Location};
use crate::config::{alias, locator};
use crate::output::{Format, Output};

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OutputFormat {
    /// Human-readable alias listing
    Text,
    /// Compact JSON grouped by network passphrase
    Json,
    /// Indented JSON grouped by network passphrase
    JsonFormatted,
}

impl From<OutputFormat> for Format {
    fn from(format: OutputFormat) -> Self {
        match format {
            OutputFormat::Text => Self::Readable,
            OutputFormat::Json => Self::Json,
            OutputFormat::JsonFormatted => Self::JsonFormatted,
        }
    }
}

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub config_locator: locator::Args,

    /// Name of the configured network to list
    #[arg(long)]
    network: Option<String>,

    /// Format of the output
    #[arg(long, value_enum, default_value = "text")]
    output: OutputFormat,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Locator(#[from] locator::Error),

    #[error(transparent)]
    Network(#[from] network::Error),

    #[error(transparent)]
    IoError(#[from] std::io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Serialize)]
struct AliasEntry {
    alias: String,
    contract: String,
    builtin: bool,
    disabled: bool,
}

impl Cmd {
    pub fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let config_dirs = self.config_locator.local_and_global()?;
        let output = Output::new(self.output.into(), global_args.quiet);

        for cfg in config_dirs {
            match cfg {
                Location::Local(config_dir) => {
                    if config_dir.exists() {
                        print_deprecation_warning(&config_dir);
                    }
                }
                Location::Global(config_dir) => self.read_from_config_dir(&config_dir, &output)?,
            }
        }

        Ok(())
    }

    fn collect_aliases(config_dir: &Path) -> Result<BTreeMap<String, Vec<AliasEntry>>, Error> {
        let contract_ids_dir = config_dir.join("contract-ids");
        let mut map: BTreeMap<String, Vec<AliasEntry>> = BTreeMap::new();

        if !contract_ids_dir.is_dir() {
            return Ok(map);
        }

        for entry in fs::read_dir(&contract_ids_dir)? {
            let path = entry?.path();

            if path.extension() != Some(OsStr::new("json")) {
                continue;
            }

            if let Some(alias) = path.file_stem() {
                let alias = alias.to_string_lossy().into_owned();
                let content = fs::read_to_string(&path)?;
                let data: alias::Data = serde_json::from_str(&content).unwrap_or_default();

                for (network_passphrase, contract_id) in &data.ids {
                    let entry = AliasEntry {
                        alias: alias.clone(),
                        contract: contract_id.clone(),
                        builtin: false,
                        disabled: alias::is_reserved(&alias),
                    };

                    map.entry(network_passphrase.clone())
                        .or_default()
                        .push(entry);
                }
            }
        }

        Ok(map)
    }

    fn read_from_config_dir(&self, config_dir: &Path, output: &Output) -> Result<(), Error> {
        let mut map = Self::collect_aliases(config_dir)?;

        if let Some(name) = &self.network {
            let passphrase = self.config_locator.read_network(name)?.network_passphrase;
            map.retain(|network, _| *network == passphrase);
            map.entry(passphrase).or_default();
        } else {
            // Include known networks even when they only have the built-in native alias.
            for (_, network, _) in self.config_locator.list_networks_long()? {
                map.entry(network.network_passphrase).or_default();
            }
        }

        for (network_passphrase, list) in &mut map {
            if let Some(contract) =
                alias::resolve_reserved(alias::NATIVE, &self.config_locator, network_passphrase)
            {
                list.push(AliasEntry {
                    alias: alias::NATIVE.to_string(),
                    contract: format!("{contract}"),
                    builtin: true,
                    disabled: false,
                });
            }

            list.sort_by(|a, b| a.alias.cmp(&b.alias));
        }

        output.readable(|_| {
            for (network_passphrase, list) in &map {
                println!("ℹ️ Aliases available for network '{network_passphrase}'");
                for entry in list {
                    let note = if entry.builtin {
                        " (built-in)"
                    } else if entry.disabled {
                        " (disabled)"
                    } else {
                        ""
                    };
                    println!("{}: {}{note}", entry.alias, entry.contract);
                }

                println!();
            }
        });
        output.json_value(&map)?;

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
