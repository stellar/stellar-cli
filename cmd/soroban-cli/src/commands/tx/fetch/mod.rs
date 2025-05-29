use clap::{command, Parser, Subcommand};
use std::fmt::Debug;

use crate::{
    commands::global,
    config::network,
    rpc,
    xdr::{self, Hash, Limits, WriteXdr},
};

mod envelope;
mod meta;
mod result;
mod args;



#[derive(Debug, clap::Args)]
#[command(args_conflicts_with_subcommands = true)]
pub struct Cmd {
    #[command(subcommand)]
    subcommand: Option<FetchCommands>,

    #[command(flatten)]
    default: DefaultArgs,
}

#[derive(Debug, Subcommand)]
pub enum FetchCommands {
    /// Fetch the transaction result
    Result(result::Cmd),
    /// Fetch the transaction meta
    Meta(meta::Cmd),
    /// Fetch the transaction envelope
    Envelope(envelope::Cmd),
}

#[derive(Debug, clap::Args)]
struct DefaultArgs {
    #[arg(long)]
    pub hash: Option<Hash>,

    #[command(flatten)]
    pub network: Option<network::Args>,

    /// Format of the output
    #[arg(long, default_value = "json")]
    pub output: Option<args::OutputFormat>,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Result(#[from] result::Error),
    #[error(transparent)]
    Meta(#[from] meta::Error),
    #[error(transparent)]
    Envelope(#[from] envelope::Error),
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        match &self.subcommand {
            Some(FetchCommands::Result(cmd)) => cmd.run(global_args).await?,
            Some(FetchCommands::Meta(cmd)) => cmd.run(global_args).await?,
            Some(FetchCommands::Envelope(cmd)) => cmd.run(global_args).await?,
            None => {
                envelope::Cmd{ 
                    hash: self.default.hash.clone().unwrap(), 
                    network: self.default.network.clone().unwrap(),
                    output: self.default.output.clone().unwrap()
                }.run(global_args).await?
                
            },
        }
        Ok(())
    }
}
