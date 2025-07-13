use crate::{
    commands::global,
    xdr::{self, Limits, WriteXdr},
};
use clap::{command, Parser};

use super::args;

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    args: args::Args,

    /// Format of the output
    #[arg(long, default_value = "json")]
    output: args::OutputFormat,
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
        if let Some(meta) = resp.result_meta {
            match self.output {
                args::OutputFormat::Json => {
                    println!("{}", serde_json::to_string(&meta)?);
                }
                args::OutputFormat::Xdr => {
                    let meta_xdr = meta.to_xdr_base64(Limits::none()).unwrap();
                    println!("{meta_xdr}");
                }
                args::OutputFormat::JsonFormatted => {
                    println!("{}", serde_json::to_string_pretty(&meta)?);
                }
            }
        }
        Ok(())
    }
}
