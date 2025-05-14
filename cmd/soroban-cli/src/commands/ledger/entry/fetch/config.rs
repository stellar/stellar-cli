use super::args::Args;
use crate::xdr::{ConfigSettingId, LedgerKey, LedgerKeyConfigSetting};
use clap::{command, Parser};
use std::collections::HashMap;
use std::fmt::Debug;

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    /// Defines the network configuration to fetch
    #[arg(long_help = long_help() )]
    pub config_setting_ids: Option<Vec<i32>>,

    #[command(flatten)]
    pub args: Args,
}

fn long_help() -> String {
    let mut config_settings = ConfigSettingId::variants();
    config_settings.sort_by_key(|v| *v as i32);

    let config_setting_strings: Vec<String> = config_settings
        .iter()
        .map(|v| format!("{} => {:?}", *v as i32, v))
        .collect();

    let setting_options = config_setting_strings.join("\n");

    format!("Valid config setting IDs (Config Setting ID => Name):\n{setting_options}",)
}

fn config_setting_variants_to_ids() -> HashMap<ConfigSettingId, i32> {
    ConfigSettingId::variants()
        .iter()
        .map(|v| (*v, *v as i32))
        .collect()
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    // #[error(transparent)]
    // Config(#[from] config::key::Error),
    // #[error(transparent)]
    // Locator(#[from] locator::Error),
    // #[error(transparent)]
    // Network(#[from] network::Error),
    // #[error(transparent)]
    // Rpc(#[from] rpc::Error),
    // #[error(transparent)]
    // Serde(#[from] serde_json::Error),
    #[error("provided config id is invalid: {0}")]
    InvalidConfigId(i32),
    #[error(transparent)]
    Run(#[from] super::args::Error),
}

impl Cmd {
    pub async fn run(&self) -> Result<(), Error> {
        let mut ledger_keys = vec![];
        self.insert_keys(&mut ledger_keys)?;
        Ok(self.args.run(ledger_keys).await?)
    }

    fn insert_keys(&self, ledger_keys: &mut Vec<LedgerKey>) -> Result<(), Error> {
        if let Some(config_setting_id) = &self.config_setting_ids {
            for x in config_setting_id {
                let key = LedgerKey::ConfigSetting(LedgerKeyConfigSetting {
                    config_setting_id: ConfigSettingId::try_from(*x)
                        .map_err(|_| Error::InvalidConfigId(*x))?,
                });
                ledger_keys.push(key);
            }
        } else {
            for (_, d) in config_setting_variants_to_ids() {
                let key = LedgerKey::ConfigSetting(LedgerKeyConfigSetting {
                    config_setting_id: ConfigSettingId::try_from(d)
                        .map_err(|_| Error::InvalidConfigId(d))?,
                });
                ledger_keys.push(key);
            }
        }

        Ok(())
    }
}
