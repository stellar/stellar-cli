use crate::{
    config::{self, locator, network},
    rpc,
    xdr::{ConfigSettingId, LedgerKey, LedgerKeyConfigSetting},
};
use clap::{command, Parser};
use std::collections::HashMap;
use std::fmt::Debug;
use super::OutputFormat;

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub network: network::Args,

    #[command(flatten)]
    pub locator: locator::Args,

    /// Defines the network configuration to fetch
    #[arg(long_help = long_help() )]
    pub config_setting_ids: Option<Vec<i32>>,

    /// Format of the output
    #[arg(long, default_value = "json")]
    pub output: OutputFormat,
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
    #[error(transparent)]
    Config(#[from] config::key::Error),
    #[error(transparent)]
    Locator(#[from] locator::Error),
    #[error(transparent)]
    Network(#[from] network::Error),
    #[error(transparent)]
    Rpc(#[from] rpc::Error),
    #[error(transparent)]
    Serde(#[from] serde_json::Error),
    #[error("provided config id is invalid: {0}")]
    InvalidConfigId(i32),
}

impl Cmd {
    pub async fn run(&self) -> Result<(), Error> {
        let network = self.network.get(&self.locator)?;
        let client = network.rpc_client()?;
        let mut ledger_keys = vec![];

        self.insert_keys(&mut ledger_keys)?;

        match self.output {
            OutputFormat::Json => {
                let resp = client.get_full_ledger_entries(&ledger_keys).await?;
                println!("{}", serde_json::to_string(&resp)?);
            }
            OutputFormat::Xdr => {
                let resp = client.get_ledger_entries(&ledger_keys).await?;
                println!("{}", serde_json::to_string(&resp)?);
            }
            OutputFormat::JsonFormatted => {
                let resp = client.get_full_ledger_entries(&ledger_keys).await?;
                println!("{}", serde_json::to_string_pretty(&resp)?);
            }
        }

        Ok(())
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
