use hex::ToHex;
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::commands::global;
use crate::config;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Config(#[from] config::Error),

    #[error(transparent)]
    Network(#[from] config::network::Error),

    #[error(transparent)]
    Serde(#[from] serde_json::Error),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, clap::ValueEnum, Default)]
pub enum OutputFormat {
    /// Plain text output of the network ID
    #[default]
    Plain,
    /// JSON output including the network passphrase
    Json,
    /// Formatted (multiline) JSON output including the network passphrase
    JsonFormatted,
}

#[derive(Debug, clap::Parser, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub config: config::ArgsLocatorAndNetwork,
    /// Format of the output
    #[arg(long, default_value = "plain")]
    pub output: OutputFormat,
}

#[derive(Serialize)]
struct JsonOutput {
    id: String,
    network_passphrase: String,
}

impl Cmd {
    pub fn run(&self, _global_args: &global::Args) -> Result<(), Error> {
        let network = self.config.get_network()?;
        let hash: String = Sha256::digest(network.network_passphrase.as_bytes()).encode_hex();

        match self.output {
            OutputFormat::Plain => println!("{hash}"),
            OutputFormat::Json => println!(
                "{}",
                serde_json::to_string(&JsonOutput {
                    id: hash,
                    network_passphrase: network.network_passphrase,
                })?
            ),
            OutputFormat::JsonFormatted => println!(
                "{}",
                serde_json::to_string_pretty(&JsonOutput {
                    id: hash,
                    network_passphrase: network.network_passphrase,
                })?
            ),
        }

        Ok(())
    }
}
