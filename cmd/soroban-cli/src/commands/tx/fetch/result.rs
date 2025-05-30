use super::args;
use crate::{
    commands::global,
    xdr::{self, Limits, WriteXdr},
};
use clap::{command, Parser};

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    args: args::Args,
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

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, clap::ValueEnum, Default)]
pub enum OutputFormat {
    /// JSON output of the ledger entry with parsed XDRs (one line, not formatted)
    #[default]
    Json,
    /// Formatted (multiline) JSON output of the ledger entry with parsed XDRs
    JsonFormatted,
    /// Original RPC output (containing XDRs)
    Xdr,
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let resp = self.args.fetch_transaction(global_args).await?;
        if let Some(result) = resp.result {
            match self.args.output {
                args::OutputFormat::Json => {
                    println!("{}", serde_json::to_string(&result)?);
                }
                args::OutputFormat::Xdr => {
                    let result_xdr = result.to_xdr_base64(Limits::none()).unwrap();
                    println!("{}", &result_xdr);
                }
                args::OutputFormat::JsonFormatted => {
                    println!("{}", serde_json::to_string_pretty(&result)?);
                }
            }
        }

        Ok(())
    }
}
