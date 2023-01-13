use clap::{AppSettings, CommandFactory, FromArgMatches, Parser};

pub mod completion;
pub mod contract;
pub mod events;
pub mod jsonrpc;
pub mod lab;
pub mod network;
pub mod rpc;
pub mod serve;
pub mod strval;
pub mod toid;
pub mod utils;
pub mod version;

pub const HEADING_SANDBOX: &str = "OPTIONS (SANDBOX)";
pub const HEADING_RPC: &str = "OPTIONS (RPC)";

#[derive(Parser, Debug)]
#[clap(
    name = "soroban",
    version,
    about = "https://soroban.stellar.org",
    disable_help_subcommand = true,
    disable_version_flag = true
)]
#[clap(global_setting(AppSettings::DeriveDisplayOrder))]
pub struct Root {
    #[clap(subcommand)]
    cmd: Cmd,
}

#[derive(Parser, Debug)]
pub enum Cmd {
    /// Tools for smart contract developers
    #[clap(subcommand)]
    Contract(contract::SubCmd),
    /// Run a local webserver for web app development and testing
    Serve(serve::Cmd),
    /// Watch the network for contract events
    Events(events::Cmd),
    /// Experiment with early features and expert tools
    #[clap(subcommand)]
    Lab(lab::SubCmd),
    /// Print version information
    Version(version::Cmd),
    /// Print shell completion code for the specified shell.
    #[clap(long_about = completion::LONG_ABOUT)]
    Completion(completion::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum CmdError {
    // TODO: stop using Debug for displaying errors
    #[error(transparent)]
    Contract(#[from] contract::Error),
    #[error(transparent)]
    Events(#[from] events::Error),
    #[error(transparent)]
    Serve(#[from] serve::Error),
    #[error(transparent)]
    Lab(#[from] lab::Error),
}

async fn run(cmd: Cmd) -> Result<(), CmdError> {
    match cmd {
        Cmd::Contract(contract) => contract.run().await?,
        Cmd::Events(events) => events.run().await?,
        Cmd::Serve(serve) => serve.run().await?,
        Cmd::Lab(lab) => lab.run().await?,
        Cmd::Version(version) => version.run(),
        Cmd::Completion(completion) => completion.run(),
    };
    Ok(())
}

#[tokio::main]
async fn main() {
    // We expand the Root::parse() invocation, so that we can save
    // Clap's ArgMatches (for later argument processing)
    let mut matches = Root::command().get_matches();
    let root = Root::from_arg_matches_mut(&mut matches).unwrap_or_else(|e| {
        let mut cmd = Root::command();
        e.format(&mut cmd).exit();
    });

    if let Err(e) = run(root.cmd).await {
        eprintln!("error: {e}");
    }
}
