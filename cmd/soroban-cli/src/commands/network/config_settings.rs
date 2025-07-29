use crate::config;
use crate::config::network;
use clap::command;
use stellar_xdr::curr::{
    ConfigSettingId, ConfigUpgradeSet, LedgerEntryData, LedgerKey, LedgerKeyConfigSetting,
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Config(#[from] config::Error),
    #[error(transparent)]
    Network(#[from] network::Error),
    #[error(transparent)]
    Serde(#[from] serde_json::Error),
    #[error(transparent)]
    Rpc(#[from] soroban_rpc::Error),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, clap::ValueEnum, Default)]
pub enum OutputFormat {
    /// JSON result of the RPC request
    #[default]
    Json,
    /// Formatted (multiline) JSON output of the RPC request
    JsonFormatted,
}

#[derive(Debug, clap::Parser, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub config: config::ArgsLocatorAndNetwork,
    /// Format of the output
    #[arg(long, default_value = "json")]
    pub output: OutputFormat,
}

impl Cmd {
    pub async fn run(&self) -> Result<(), Error> {
        let keys = self.config_settings_keys();
        let settings = self
            .config
            .get_network()?
            .rpc_client()?
            .get_full_ledger_entries(&keys)
            .await?
            .entries
            .into_iter()
            .filter_map(|e| match e.val {
                LedgerEntryData::ConfigSetting(setting) => Some(setting),
                _ => None,
            })
            .collect::<Vec<_>>();
        let set = ConfigUpgradeSet {
            updated_entry: settings.try_into().unwrap(),
        };
        match self.output {
            OutputFormat::Json => println!("{}", serde_json::to_string(&set)?),
            OutputFormat::JsonFormatted => {
                println!("{}", serde_json::to_string_pretty(&set)?)
            }
        }
        Ok(())
    }

    fn config_settings_keys(&self) -> Vec<LedgerKey> {
        ConfigSettingId::variants()
            .into_iter()
            .map(|id| {
                LedgerKey::ConfigSetting(LedgerKeyConfigSetting {
                    config_setting_id: id,
                })
            })
            .collect()
    }
}
