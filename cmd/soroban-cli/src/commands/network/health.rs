use crate::commands::global;
use crate::config::network;
use crate::{config, print};
use clap::command;

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
    /// Text output of network health status
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

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let print = print::Print::new(global_args.quiet);
        let result = self.config.get_network()?.rpc_client()?.get_health().await;

        match result {
            Ok(resp) => match self.output {
                OutputFormat::Text => {
                    print.emoji_println('🟢', "Healthy");
                    println!("Latest ledger: {}", resp.latest_ledger);
                }
                OutputFormat::Json => println!("{}", serde_json::to_string(&resp)?),
                OutputFormat::JsonFormatted => println!("{}", serde_json::to_string_pretty(&resp)?),
            },
            Err(err) => {
                print.errorln(format!("failed to fetch network health: {err}"));
                print.emoji_println('🔴', "Unhealthy");
            }
        }

        Ok(())
    }
}
