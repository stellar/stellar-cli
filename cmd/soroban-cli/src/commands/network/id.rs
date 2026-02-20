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
    /// Text output of the network ID
    #[default]
    Text,
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
    #[arg(long, default_value = "text")]
    pub output: OutputFormat,
}

#[derive(Serialize)]
struct JsonOutput {
    id: String,
    network_passphrase: String,
}

impl Cmd {
    pub fn run(&self, _global_args: &global::Args) -> Result<(), Error> {
        let network_passphrase = self.config.get_passphrase()?;
        let hash = hex::encode(Sha256::digest(network_passphrase.as_bytes()));

        match self.output {
            OutputFormat::Text => println!("{hash}"),
            OutputFormat::Json => println!(
                "{}",
                serde_json::to_string(&JsonOutput {
                    id: hash,
                    network_passphrase,
                })?
            ),
            OutputFormat::JsonFormatted => println!(
                "{}",
                serde_json::to_string_pretty(&JsonOutput {
                    id: hash,
                    network_passphrase,
                })?
            ),
        }

        Ok(())
    }
}
