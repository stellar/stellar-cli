use crate::commands::global;
use crate::config::network;
use crate::rpc::FullLedgerEntries;
use crate::{config, print};
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
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let print = print::Print::new(global_args.quiet);
        let result = self
            .config
            .get_network()?
            .rpc_client()?
            .get_full_ledger_entries(&self.config_settings_keys())
            .await;

        match result {
            Ok(FullLedgerEntries { entries, .. }) => {
                let entries = entries
                    .into_iter()
                    .filter_map(|e| match e.val {
                        LedgerEntryData::ConfigSetting(setting) => Some(setting),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                let set = ConfigUpgradeSet {
                    updated_entry: entries.try_into().unwrap(),
                };
                match self.output {
                    OutputFormat::Json => println!("{}", serde_json::to_string(&set)?),
                    OutputFormat::JsonFormatted => {
                        println!("{}", serde_json::to_string_pretty(&set)?)
                    }
                }
            }
            Err(err) => {
                print.errorln(format!("failed to fetch network config settings: {err}"));
                return Err(err.into());
            }
        }
    }
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
