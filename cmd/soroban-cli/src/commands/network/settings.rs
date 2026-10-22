use crate::config::network;
use crate::output::{Format, Output};
use crate::utils::XDR_DEPTH_LIMIT;
use crate::{commands::global, config};
use semver::Version;
use stellar_xdr::{
    ConfigSettingId, ConfigUpgradeSet, LedgerEntryData, LedgerKey, LedgerKeyConfigSetting, Limits,
    WriteXdr as _,
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Config(#[from] config::Error),
    #[error(transparent)]
    Network(#[from] network::Error),
    #[error(transparent)]
    Xdr(#[from] stellar_xdr::Error),
    #[error(transparent)]
    Serde(#[from] serde_json::Error),
    #[error(transparent)]
    Rpc(#[from] soroban_rpc::Error),
    #[error(transparent)]
    Semver(#[from] semver::Error),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, clap::ValueEnum, Default)]
pub enum OutputFormat {
    /// XDR (`ConfigUpgradeSet` type)
    Xdr,
    /// JSON, XDR-JSON of the `ConfigUpgradeSet` XDR type
    #[default]
    Json,
    /// JSON formatted, XDR-JSON of the `ConfigUpgradeSet` XDR type
    JsonFormatted,
}

impl From<OutputFormat> for Format {
    fn from(value: OutputFormat) -> Self {
        match value {
            // Xdr is a raw, human-facing rendering handled outside the JSON path.
            OutputFormat::Xdr => Format::Readable,
            OutputFormat::Json => Format::Json,
            OutputFormat::JsonFormatted => Format::JsonFormatted,
        }
    }
}

#[derive(Debug, clap::Parser, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub config: config::ArgsLocatorAndNetwork,
    /// Include internal config settings that are not upgradeable and are internally maintained by
    /// the network
    #[arg(long)]
    pub internal: bool,
    /// Include verbose contract cost parameters (CPU instructions and memory bytes)
    #[arg(long)]
    pub include_costs: bool,
    /// Format of the output
    #[arg(long, default_value = "json")]
    pub output: OutputFormat,
}

impl Cmd {
    pub fn collect_keys(internal: bool, include_costs: bool) -> Vec<LedgerKey> {
        ConfigSettingId::variants()
            .into_iter()
            .filter(|id| match id {
                // Internally maintained settings that a network validator cannot vote to change
                // are not output by this command unless the internal option is specified.
                ConfigSettingId::LiveSorobanStateSizeWindow | ConfigSettingId::EvictionIterator => {
                    internal
                }
                // Verbose contract cost params (CPU instructions and memory bytes)
                // are excluded by default unless include_costs is specified.
                ConfigSettingId::ContractCostParamsCpuInstructions
                | ConfigSettingId::ContractCostParamsMemoryBytes => include_costs,
                // All other configs can be modified by network upgrades and are always output.
                _ => true,
            })
            .map(|id| {
                LedgerKey::ConfigSetting(LedgerKeyConfigSetting {
                    config_setting_id: id,
                })
            })
            .collect()
    }

    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let output = Output::new(self.output.into(), global_args.quiet);
        let rpc = self.config.get_network()?.rpc_client()?;

        // If the network protocol version is ahead of the XDR version (which tracks the protocol
        // version), there could be config settings defined in the newer protocol version that the
        // CLI doesn't know about. Warn, because the output of this command might provide an
        // incomplete view of the network's config settings.
        let network_version = rpc.get_version_info().await?.protocol_version;
        let self_version = Version::parse(stellar_xdr::VERSION.pkg)?.major;
        if self_version < network_version.into() {
            // Diagnostic about data completeness; emitted regardless of format
            // (to stderr) so JSON consumers reading stdout are unaffected.
            output.print().warnln(format!("Network protocol version is {network_version} but the stellar-cli supports {self_version}. The config fetched may not represent the complete config settings for the network. Upgrade the stellar-cli."));
        }

        // Collect the ledger entries for all the config settings.
        let keys = Self::collect_keys(self.internal, self.include_costs);
        let settings = rpc
            .get_full_ledger_entries(&keys)
            .await?
            .entries
            .into_iter()
            .filter_map(|e| match e.val {
                LedgerEntryData::ConfigSetting(setting) => Some(setting),
                _ => None,
            })
            .collect::<Vec<_>>();

        let config_upgrade_set = ConfigUpgradeSet {
            updated_entry: settings.try_into().unwrap(),
        };
        match self.output {
            // Xdr is a distinct raw rendering, handled outside the JSON path.
            OutputFormat::Xdr => println!(
                "{}",
                config_upgrade_set.to_xdr_base64(Limits::depth(XDR_DEPTH_LIMIT))?
            ),
            OutputFormat::Json | OutputFormat::JsonFormatted => {
                output.json_value(&config_upgrade_set)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collect_keys_default_excludes_costs_and_internal() {
        let keys = Cmd::collect_keys(false, false);
        for key in &keys {
            if let LedgerKey::ConfigSetting(setting) = key {
                assert_ne!(
                    setting.config_setting_id,
                    ConfigSettingId::ContractCostParamsCpuInstructions,
                    "Default keys must exclude CPU cost params"
                );
                assert_ne!(
                    setting.config_setting_id,
                    ConfigSettingId::ContractCostParamsMemoryBytes,
                    "Default keys must exclude Memory cost params"
                );
                assert_ne!(
                    setting.config_setting_id,
                    ConfigSettingId::LiveSorobanStateSizeWindow,
                    "Default keys must exclude internal LiveSorobanStateSizeWindow"
                );
                assert_ne!(
                    setting.config_setting_id,
                    ConfigSettingId::EvictionIterator,
                    "Default keys must exclude internal EvictionIterator"
                );
            }
        }
    }

    #[test]
    fn test_collect_keys_include_costs() {
        let keys = Cmd::collect_keys(false, true);
        let has_cpu_costs = keys.iter().any(|k| match k {
            LedgerKey::ConfigSetting(s) => {
                s.config_setting_id == ConfigSettingId::ContractCostParamsCpuInstructions
            }
            _ => false,
        });
        let has_mem_costs = keys.iter().any(|k| match k {
            LedgerKey::ConfigSetting(s) => {
                s.config_setting_id == ConfigSettingId::ContractCostParamsMemoryBytes
            }
            _ => false,
        });
        let has_internal = keys.iter().any(|k| match k {
            LedgerKey::ConfigSetting(s) => {
                s.config_setting_id == ConfigSettingId::LiveSorobanStateSizeWindow
                    || s.config_setting_id == ConfigSettingId::EvictionIterator
            }
            _ => false,
        });

        assert!(
            has_cpu_costs,
            "Keys with include_costs=true must include CPU cost params"
        );
        assert!(
            has_mem_costs,
            "Keys with include_costs=true must include Memory cost params"
        );
        assert!(
            !has_internal,
            "Keys with internal=false must not include internal settings"
        );
    }

    #[test]
    fn test_collect_keys_include_internal() {
        let keys = Cmd::collect_keys(true, false);
        let has_internal_window = keys.iter().any(|k| match k {
            LedgerKey::ConfigSetting(s) => {
                s.config_setting_id == ConfigSettingId::LiveSorobanStateSizeWindow
            }
            _ => false,
        });
        let has_internal_eviction = keys.iter().any(|k| match k {
            LedgerKey::ConfigSetting(s) => s.config_setting_id == ConfigSettingId::EvictionIterator,
            _ => false,
        });
        let has_cpu_costs = keys.iter().any(|k| match k {
            LedgerKey::ConfigSetting(s) => {
                s.config_setting_id == ConfigSettingId::ContractCostParamsCpuInstructions
            }
            _ => false,
        });

        assert!(
            has_internal_window,
            "Keys with internal=true must include LiveSorobanStateSizeWindow"
        );
        assert!(
            has_internal_eviction,
            "Keys with internal=true must include EvictionIterator"
        );
        assert!(
            !has_cpu_costs,
            "Keys with include_costs=false must not include cost params"
        );
    }

    #[test]
    fn test_collect_keys_include_all() {
        let keys = Cmd::collect_keys(true, true);
        assert_eq!(
            keys.len(),
            ConfigSettingId::variants().len(),
            "When internal=true and include_costs=true, all variants must be included"
        );
    }
}
