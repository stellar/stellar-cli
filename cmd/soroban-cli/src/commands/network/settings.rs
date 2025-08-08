use crate::config::network;
use crate::print::Print;
use crate::{commands::global, config};
use clap::command;
use semver::Version;
use stellar_xdr::curr::{
    ConfigSettingId, ConfigUpgradeSet, LedgerEntryData, LedgerKey, LedgerKeyConfigSetting, Limits,
    WriteXdr as _,
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Config(#[from] config::Error),
    #[error(transparent)]
    Network(Box<network::Error>),
    #[error(transparent)]
    Xdr(#[from] stellar_xdr::curr::Error),
    #[error(transparent)]
    Serde(#[from] serde_json::Error),
    #[error(transparent)]
    Rpc(Box<soroban_rpc::Error>),
    #[error(transparent)]
    Semver(#[from] semver::Error),
}

impl From<network::Error> for Error {
    fn from(e: network::Error) -> Self {
        Self::Network(Box::new(e))
    }
}

impl From<soroban_rpc::Error> for Error {
    fn from(e: soroban_rpc::Error) -> Self {
        Self::Rpc(Box::new(e))
    }
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

#[derive(Debug, clap::Parser, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub config: config::ArgsLocatorAndNetwork,
    /// Include internal config settings that are not upgradeable and are internally maintained by
    /// the network
    #[arg(long)]
    pub internal: bool,
    /// Format of the output
    #[arg(long, default_value = "json")]
    pub output: OutputFormat,
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let print = Print::new(global_args.quiet);
        let rpc = self.config.get_network()?.rpc_client()?;

        // If the network protocol version is ahead of the XDR version (which tracks the protocol
        // version), there could be config settings defined in the newer protocol version that the
        // CLI doesn't know about. Warn, because the output of this command might provide an
        // incomplete view of the network's config settings.
        let network_version = rpc.get_version_info().await?.protocol_version;
        let self_version = Version::parse(stellar_xdr::VERSION.pkg)?.major;
        if self_version < network_version.into() {
            print.warnln(format!("Network protocol version is {network_version} but the stellar-cli supports {self_version}. The config fetched may not represent the complete config settings for the network. Upgrade the stellar-cli."));
        }

        // Collect the ledger entries for all the config settings.
        let keys = ConfigSettingId::variants()
            .into_iter()
            .filter(|id| match id {
                // Internally maintained settings that a network validator cannot vote to change
                // are not output by this command unless the internal option is specified.
                ConfigSettingId::LiveSorobanStateSizeWindow | ConfigSettingId::EvictionIterator => {
                    self.internal
                }
                // All other configs can be modified by network upgrades and are always output.
                _ => true,
            })
            .map(|id| {
                LedgerKey::ConfigSetting(LedgerKeyConfigSetting {
                    config_setting_id: id,
                })
            })
            .collect::<Vec<_>>();
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
            OutputFormat::Xdr => println!("{}", config_upgrade_set.to_xdr_base64(Limits::none())?),
            OutputFormat::Json => println!("{}", serde_json::to_string(&config_upgrade_set)?),
            OutputFormat::JsonFormatted => {
                println!("{}", serde_json::to_string_pretty(&config_upgrade_set)?);
            }
        }
        Ok(())
    }
}
