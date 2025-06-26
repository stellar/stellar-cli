use crate::{commands::global, config::network, rpc};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Serde(#[from] serde_json::Error),
    #[error(transparent)]
    Network(#[from] network::Error),
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

#[derive(Debug, clap::Parser)]
pub struct Cmd {
    #[command(flatten)]
    pub network: network::Args,

    /// Format of the output
    #[arg(long, value_enum, default_value_t)]
    pub output: OutputFormat,
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let network = self.network.get(&global_args.locator)?;
        let client = network.rpc_client()?;
        let fee_stats = client.get_fee_stats().await?;

        match self.output {
            OutputFormat::Text => {
                println!(
                    "Max Soroban Inclusion Fee: {}",
                    fee_stats.soroban_inclusion_fee.max
                );
                println!("Max Inclusion Fee: {}", fee_stats.inclusion_fee.max);
                println!("Latest Ledger: {}", fee_stats.latest_ledger);
                println!("\nFor more details use --output flag with 'json' or 'json-formatted'");
            }
            OutputFormat::Json => println!("{}", serde_json::to_string(&fee_stats)?),
            OutputFormat::JsonFormatted => {
                println!("{}", serde_json::to_string_pretty(&fee_stats)?);
            }
        }

        Ok(())
    }
}
