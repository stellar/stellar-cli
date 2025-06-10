use crate::commands::global;
use crate::config::network;
use crate::{config, print, rpc};
use clap::command;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Config(#[from] config::Error),
    #[error(transparent)]
    Network(#[from] network::Error),
    #[error(transparent)]
    Serde(#[from] serde_json::Error),
    #[error(transparent)]
    Rpc(#[from] rpc::Error),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, clap::ValueEnum, Default)]
pub enum OutputFormat {
    /// JSON result of the RPC request
    Json,
    /// Formatted (multiline) JSON output of the RPC request
    #[default]
    JsonFormatted,
}

#[derive(Debug, clap::Parser, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub config: config::ArgsLocatorAndNetwork,
    /// Format of the output
    #[arg(long, default_value = "json-formatted")]
    pub output: OutputFormat,
}

#[derive(serde::Deserialize, serde::Serialize)]
struct Info {
    pub version: String,
    pub commmit_hash: String,
    pub build_timestamp: String,
    pub captive_core_version: String,
    pub protocol_version: u32,
    pub friendbot_url: Option<String>,
    pub passphrase: String,
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let print = print::Print::new(global_args.quiet);
        let rpc_client = self.config.get_network()?.rpc_client()?;
        let network_result = rpc_client.get_network().await?;
        let version_result = rpc_client.get_version_info().await?;
        println!("version_result: {version_result:?}");
        let info = Info {
            version: version_result.version,
            commmit_hash: version_result.commmit_hash,
            build_timestamp: version_result.build_timestamp,
            captive_core_version: version_result.captive_core_version,
            protocol_version: network_result.protocol_version,
            friendbot_url: network_result.friendbot_url,
            passphrase: network_result.passphrase,
        };

        match self.output {
            OutputFormat::Json => println!("{}", serde_json::to_string(&info)?),
            OutputFormat::JsonFormatted => println!("{}", serde_json::to_string_pretty(&info)?),
        }

        Ok(())
    }
}
