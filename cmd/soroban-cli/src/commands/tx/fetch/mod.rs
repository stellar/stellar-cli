use clap::{command, Subcommand};
use std::fmt::Debug;

use crate::{commands::global, config::network, xdr::Hash};

mod args;
mod envelope;
mod meta;
mod result;

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
    #[command(hide = true)]
    Envelope(envelope::Cmd),
}

#[derive(Debug, clap::Args)]
struct DefaultArgs {
    /// Hash of transaction to fetch
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
                envelope::Cmd {
                    args: args::Args {
                        hash: self.default.hash.clone().expect("Transaction hash is required but was not provided."),
                        network: self.default.network.clone().unwrap(),
                        output: self.default.output.unwrap(),
                    },
                }
                .run(global_args)
                .await?;
            }
        }
        Ok(())
    }
}
