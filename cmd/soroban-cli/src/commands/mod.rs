use std::str::FromStr;

use async_trait::async_trait;
use clap::{command, error::ErrorKind, CommandFactory, FromArgMatches, Parser};
use futures_util::TryFutureExt;

use crate::config;

pub mod completion;
pub mod contract;
pub mod global;
pub mod plugin;
pub mod txn_result;
pub mod version;
pub mod policy;

pub const HEADING_RPC: &str = "Options (RPC)";
pub const HEADING_GLOBAL: &str = "Options (Global)";
pub const ABOUT: &str =
    "Work seamlessly with Stellar accounts, contracts, and assets from the command line.

- Generate and manage keys and accounts
- Build, deploy, and interact with contracts
- Deploy asset contracts
- Stream events
- Start local testnets
- Decode, encode XDR
- More!

For additional information see:

- Stellar Docs: https://developers.stellar.org
- Smart Contract Docs: https://developers.stellar.org/docs/build/smart-contracts/overview
- CLI Docs: https://developers.stellar.org/docs/tools/developer-tools/cli/stellar-cli";

pub const LONG_ABOUT: &str = "

To get started generate a new identity:

    stellar keys generate alice

Use keys with the `--source` flag in other commands.

Commands that work with contracts are organized under the `contract` subcommand. List them:

    stellar contract --help

Use contracts like a CLI:

    stellar contract invoke --id CCR6QKTWZQYW6YUJ7UP7XXZRLWQPFRV6SWBLQS4ZQOSAF4BOUD77OTE2 --source alice --network testnet -- --help

Anything after the `--` double dash (the \"slop\") is parsed as arguments to the contract-specific CLI, generated on-the-fly from the contract schema. For the hello world example, with a function called `hello` that takes one string argument `to`, here's how you invoke it:

    stellar contract invoke --id CCR6QKTWZQYW6YUJ7UP7XXZRLWQPFRV6SWBLQS4ZQOSAF4BOUD77OTE2 --source alice --network testnet -- hello --to world
";

#[derive(Parser, Debug)]
#[command(
    name = "stellar",
    about = ABOUT,
    version = version::long(),
    long_about = ABOUT.to_string() + LONG_ABOUT,
    disable_help_subcommand = true,
)]
pub struct Root {
    #[clap(flatten)]
    pub global_args: global::Args,

    #[command(subcommand)]
    pub cmd: Cmd,
}

impl Root {
    pub fn new() -> Result<Self, Error> {
        Self::try_parse().map_err(|e| {
            if std::env::args().any(|s| s == "--list") {
                let plugins = plugin::list().unwrap_or_default();
                if plugins.is_empty() {
                    println!("No Plugins installed. E.g. soroban-hello");
                } else {
                    println!("Installed Plugins:\n    {}", plugins.join("\n    "));
                }
                std::process::exit(0);
            }
            match e.kind() {
                ErrorKind::InvalidSubcommand => match plugin::run() {
                    Ok(()) => Error::Clap(e),
                    Err(e) => Error::Plugin(e),
                },
                _ => Error::Clap(e),
            }
        })
    }

    pub fn from_arg_matches<I, T>(itr: I) -> Result<Self, clap::Error>
    where
        I: IntoIterator<Item = T>,
        T: Into<std::ffi::OsString> + Clone,
    {
        Self::from_arg_matches_mut(&mut Self::command().get_matches_from(itr))
    }

    pub fn command() -> clap::Command {
        <Self as clap::CommandFactory>::command()
    }

    pub async fn run(&self) -> Result<(), Error> {
        match &self.cmd {
            Cmd::Completion(completion) => completion.run().map_err(Error::from),
            Cmd::Contract(contract) => Ok(contract.run(&self.global_args).await?),
            Cmd::Policy(policy) => policy.run().await.map_err(Error::from),
        }
    }
}

impl FromStr for Root {
    type Err = clap::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_arg_matches(s.split_whitespace())
    }
}

#[derive(Debug, clap::Subcommand)]
pub enum Cmd {
    /// Generate shell completions
    Completion(completion::Cmd),
    /// Contract commands
    #[command(subcommand)]
    Contract(contract::Cmd),
    /// Policy generator commands
    #[command(subcommand)]
    Policy(policy::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Contract(#[from] contract::Error),
    #[error(transparent)]
    Policy(#[from] policy::Error),
    #[error(transparent)]
    Plugin(#[from] plugin::Error),
    #[error(transparent)]
    Clap(#[from] clap::error::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

#[async_trait]
pub trait NetworkRunnable {
    type Error;
    type Result;

    async fn run_against_rpc_server(
        &self,
        global_args: Option<&global::Args>,
        config: Option<&config::Args>,
    ) -> Result<Self::Result, Self::Error>;
}
