use clap::{AppSettings, CommandFactory, FromArgMatches, Parser};

mod completion;
mod contract;
mod events;
mod install;
mod jsonrpc;
mod lab;
mod network;
mod rpc;
mod serve;
mod strval;
mod toid;
mod utils;
mod version;

const HEADING_SANDBOX: &str = "OPTIONS (SANDBOX)";
const HEADING_RPC: &str = "OPTIONS (RPC)";

#[derive(Parser, Debug)]
#[clap(
    name = "soroban",
    version,
    about = "https://soroban.stellar.org",
    disable_help_subcommand = true,
    disable_version_flag = true
)]
#[clap(global_setting(AppSettings::DeriveDisplayOrder))]
struct Root {
    #[clap(subcommand)]
    cmd: Cmd,
}

#[derive(Parser, Debug)]
enum Cmd {
    /// Tools for smart contract developers
    #[clap(subcommand)]
    Contract(contract::SubCmd),
    /// Run a local webserver for web app development and testing
    Serve(serve::Cmd),
    /// Watch the network for contract events
    Events(events::Cmd),
    /// Install a WASM file to the ledger without creating a contract instance
    Install(install::Cmd),
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
enum CmdError {
    // TODO: stop using Debug for displaying errors
    #[error(transparent)]
    Contract(#[from] contract::Error),
    #[error(transparent)]
    Events(#[from] events::Error),
    #[error(transparent)]
    Serve(#[from] serve::Error),
    #[error(transparent)]
    Install(#[from] install::Error),
    #[error(transparent)]
    Lab(#[from] lab::Error),
}

async fn run(cmd: Cmd, sub_arg_matches: &clap::ArgMatches) -> Result<(), CmdError> {
    match cmd {
        Cmd::Contract(contract) => contract.run(sub_arg_matches).await?,
        Cmd::Events(events) => events.run().await?,
        Cmd::Serve(serve) => serve.run().await?,
        Cmd::Install(install) => install.run().await?,
        Cmd::Lab(lab) => lab.run().await?,
        Cmd::Version(version) => version.run(),
        Cmd::Completion(completion) => completion.run(&mut Root::command()),
    };
    Ok(())
}

#[tokio::main]
async fn main() {
    // We expand the Root::parse() invocation, so that we can save
    // Clap's ArgMatches (for later argument processing)
    let mut matches = Root::command().get_matches();
    let root = match Root::from_arg_matches_mut(&mut matches) {
        Ok(s) => s,
        Err(e) => {
            let mut cmd = Root::command();
            e.format(&mut cmd).exit();
        }
    };

    let (_, sub_arg_matches) = matches.remove_subcommand().unwrap();
    if let Err(e) = run(root.cmd, &sub_arg_matches).await {
        eprintln!("error: {e}");
    }
}
