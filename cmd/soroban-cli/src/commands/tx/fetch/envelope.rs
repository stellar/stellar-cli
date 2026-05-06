use crate::{
    commands::global,
    xdr::{self, Limits, WriteXdr},
};
use clap::Parser;

use super::args;

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub(crate) args: args::Args,

    /// Format of the output
    #[arg(long, default_value = "json")]
    pub(crate) output: args::OutputFormat,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Serde(#[from] serde_json::Error),
    #[error(transparent)]
    Xdr(#[from] xdr::Error),
    #[error(transparent)]
    Args(#[from] args::Error),
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let resp = self.args.fetch_transaction(global_args).await?;
        if let Some(ref envelope) = resp.envelope {
            match self.output {
                args::OutputFormat::Json => {
                    println!("{}", serde_json::to_string(&envelope)?);
                }
                args::OutputFormat::Xdr => {
                    let envelope_xdr = envelope.to_xdr_base64(Limits::none()).unwrap();
                    println!("{envelope_xdr}");
                }
                args::OutputFormat::JsonFormatted => {
                    args::Args::print_tx_summary(&resp);
                    println!("{}", serde_json::to_string_pretty(&envelope)?);
                }
            }
        }

        Ok(())
    }
}
