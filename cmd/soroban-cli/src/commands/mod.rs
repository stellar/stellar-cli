use std::str::FromStr;

use clap::{command, CommandFactory, FromArgMatches, Parser};

pub mod completion;
pub mod config;
pub mod contract;
pub mod events;
pub mod global;
pub mod lab;
pub mod plugin;
pub mod version;

pub const HEADING_SANDBOX: &str = "Options (Sandbox)";
pub const HEADING_RPC: &str = "Options (RPC)";
const ABOUT: &str = "Build, deploy, & interact with contracts; set identities to sign with; configure networks; generate keys; and more.

Intro: https://soroban.stellar.org
CLI Reference: https://github.com/stellar/soroban-tools/tree/main/docs/soroban-cli-full-docs.md";

// long_about is shown when someone uses `--help`; short help when using `-h`
const LONG_ABOUT: &str = "

The easiest way to get started is to generate a new identity:

    soroban config identity generate alice

You can use identities with the `--source` flag in other commands later.

Commands that relate to smart contract interactions are organized under the `contract` subcommand. List them:

    soroban contract --help

A Soroban contract has its interface schema types embedded in the binary that gets deployed on-chain, making it possible to dynamically generate a custom CLI for each. `soroban contract invoke` makes use of this:

    soroban contract invoke --id 1 --source alice -- --help

Anything after the `--` double dash (the \"slop\") is parsed as arguments to the contract-specific CLI, generated on-the-fly from the embedded schema. For the hello world example, with a function called `hello` that takes one string argument `to`, here's how you invoke it:

    soroban contract invoke --id 1 --source alice -- hello --to world

Full CLI reference: https://github.com/stellar/soroban-tools/tree/main/docs/soroban-cli-full-docs.md";

#[derive(Parser, Debug)]
#[command(
    name = "soroban",
    version = version::short(),
    long_version = version::long(),
    about = ABOUT,
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
    pub fn new() -> Result<Self, clap::Error> {
        let mut matches = Self::command().get_matches();
        Self::from_arg_matches_mut(&mut matches)
    }

    pub fn from_arg_matches<I, T>(itr: I) -> Result<Self, clap::Error>
    where
        I: IntoIterator<Item = T>,
        T: Into<std::ffi::OsString> + Clone,
    {
        Self::from_arg_matches_mut(&mut Self::command().get_matches_from(itr))
    }
    pub async fn run(&mut self) -> Result<(), Error> {
        match &mut self.cmd {
            Cmd::Contract(contract) => contract.run().await?,
            Cmd::Events(events) => events.run().await?,
            Cmd::Lab(lab) => lab.run().await?,
            Cmd::Version(version) => version.run(),
            Cmd::Completion(completion) => completion.run(),
            Cmd::Config(config) => config.run()?,
        };
        Ok(())
    }
}

impl FromStr for Root {
    type Err = clap::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_arg_matches(s.split_whitespace())
    }
}

#[derive(Parser, Debug)]
pub enum Cmd {
    /// Tools for smart contract developers
    #[command(subcommand)]
    Contract(contract::Cmd),
    /// Read and update config
    #[command(subcommand)]
    Config(config::Cmd),
    /// Watch the network for contract events
    Events(events::Cmd),
    /// Experiment with early features and expert tools
    #[command(subcommand)]
    Lab(lab::Cmd),
    /// Print version information
    Version(version::Cmd),
    /// Print shell completion code for the specified shell.
    #[command(long_about = completion::LONG_ABOUT)]
    Completion(completion::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    // TODO: stop using Debug for displaying errors
    #[error(transparent)]
    Contract(#[from] contract::Error),
    #[error(transparent)]
    Events(#[from] events::Error),

    #[error(transparent)]
    Lab(#[from] lab::Error),
    #[error(transparent)]
    Config(#[from] config::Error),
}
