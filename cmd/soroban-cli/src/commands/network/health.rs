use crate::commands::global;
use crate::config::{self, network};
use crate::output::{Format, Output};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Config(#[from] config::Error),
    #[error(transparent)]
    Network(#[from] network::Error),
    #[error(transparent)]
    Rpc(#[from] crate::rpc::Error),
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

impl From<OutputFormat> for Format {
    fn from(value: OutputFormat) -> Self {
        match value {
            OutputFormat::Text => Format::Readable,
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
    /// Format of the output
    #[arg(long, default_value = "text")]
    pub output: OutputFormat,
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let output = Output::new(self.output.into(), global_args.quiet);

        // On failure, surface "Unhealthy" for humans then propagate the error so
        // the process exits non-zero and the top-level handler renders it (as the
        // structured JSON envelope in JSON mode). A reachable-but-unhealthy node
        // instead returns `Ok` with a status, handled below.
        let resp = match self.config.get_network()?.rpc_client()?.get_health().await {
            Ok(resp) => resp,
            Err(err) => {
                output.readable(|print| print.errorln("Unhealthy"));
                return Err(err.into());
            }
        };

        output.readable(|print| {
            if resp.status.eq_ignore_ascii_case("healthy") {
                print.checkln("Healthy");
            } else {
                print.warnln(format!("Status: {}", resp.status));
            }
            print.infoln(format!("Latest ledger: {}", resp.latest_ledger));
        });
        output.json_value(&resp)?;

        Ok(())
    }
}
