use crate::{
    commands::global,
    config::network,
    rpc,
    xdr::{self, Hash, Limits, WriteXdr},
};
use clap::{command, Parser};

use super::args;

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub(crate) args: args::Args
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Serde(#[from] serde_json::Error),
    #[error(transparent)]
    Xdr(#[from] xdr::Error),
    #[error(transparent)]
    Args(#[from] args::Error)
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let resp = self.args.fetch_transaction(global_args).await?;
        if let Some(envelope) = resp.envelope {
            match self.args.output {
                args::OutputFormat::Json => {
                    println!("{}", serde_json::to_string(&envelope)?);
                }
                args::OutputFormat::Xdr => {
                    let envelope_xdr = envelope.to_xdr_base64(Limits::none()).unwrap();
                    println!("{envelope_xdr}");
                }
                args::OutputFormat::JsonFormatted => {
                    println!("{}", serde_json::to_string_pretty(&envelope)?);
                }
            }

        }

        Ok(())
    }
}
