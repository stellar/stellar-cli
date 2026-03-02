use sha2::{Digest, Sha256};

use crate::commands::global;
use crate::config::network;
use crate::{config, print, rpc};

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
    /// Text output of network info
    #[default]
    Text,
    /// JSON result of the RPC request
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
    #[arg(long, default_value = "text")]
    pub output: OutputFormat,
}

#[derive(serde::Deserialize, serde::Serialize)]
struct Info {
    pub id: String,
    pub version: String,
    pub commit_hash: String,
    pub build_timestamp: String,
    pub captive_core_version: String,
    pub protocol_version: u32,
    pub passphrase: String,
    pub friendbot_url: Option<String>,
}

impl Info {
    fn print_text(&self, print: &print::Print) {
        print.infoln(format!("Network Id: {}", self.id));
        print.infoln(format!("Version: {}", self.version));
        print.infoln(format!("Commit Hash: {}", self.commit_hash));
        print.infoln(format!("Build Timestamp: {}", self.build_timestamp));
        print.infoln(format!(
            "Captive Core Version: {}",
            self.captive_core_version
        ));
        print.infoln(format!("Protocol Version: {}", self.protocol_version));
        print.infoln(format!("Passphrase: {}", self.passphrase));
        if let Some(friendbot_url) = &self.friendbot_url {
            print.infoln(format!("Friendbot Url: {friendbot_url}"));
        }
    }

    fn print_json(&self) -> Result<(), serde_json::Error> {
        let json = serde_json::to_string(&self)?;
        println!("{json}");
        Ok(())
    }

    fn print_json_formatted(&self) -> Result<(), serde_json::Error> {
        let json = serde_json::to_string_pretty(&self)?;
        println!("{json}");
        Ok(())
    }
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let print = print::Print::new(global_args.quiet);
        let rpc_client = self.config.get_network()?.rpc_client()?;
        let network_result = rpc_client.get_network().await?;
        let version_result = rpc_client.get_version_info().await?;
        let id = hex::encode(Sha256::digest(network_result.passphrase.as_bytes()));
        let info = Info {
            id,
            version: version_result.version,
            commit_hash: version_result.commmit_hash,
            build_timestamp: version_result.build_timestamp,
            captive_core_version: version_result.captive_core_version,
            protocol_version: network_result.protocol_version,
            friendbot_url: network_result.friendbot_url,
            passphrase: network_result.passphrase,
        };

        match self.output {
            OutputFormat::Text => info.print_text(&print),
            OutputFormat::Json => info.print_json()?,
            OutputFormat::JsonFormatted => info.print_json_formatted()?,
        }

        Ok(())
    }
}
