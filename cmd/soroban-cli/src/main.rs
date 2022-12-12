use clap::{AppSettings, CommandFactory, FromArgMatches, Parser, Subcommand};

mod completion;
mod deploy;
mod events;
mod gen;
mod inspect;
mod install;
mod invoke;
mod jsonrpc;
mod network;
mod optimize;
mod read;
mod rpc;
mod serve;
mod snapshot;
mod strval;
mod toid;
mod token;
mod utils;
mod version;
mod xdr;

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

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Invoke a contract function in a WASM file
    Invoke(invoke::Cmd),
    /// Inspect a WASM file listing contract functions, meta, etc
    Inspect(inspect::Cmd),
    /// Optimize a WASM file
    Optimize(optimize::Cmd),
    /// Print the current value of a contract-data ledger entry
    Read(read::Cmd),
    /// Run a local webserver for web app development and testing
    Serve(serve::Cmd),
    /// Watch the network for contract events
    Events(events::Cmd),
    /// Wrap, create, and manage token contracts
    Token(token::Root),
    /// Deploy a WASM file as a contract
    Deploy(deploy::Cmd),
    /// Install a WASM file to the ledger without creating a contract instance
    Install(install::Cmd),
    /// Generate code client bindings for a contract
    Gen(gen::Cmd),

    /// Decode xdr
    Xdr(xdr::Cmd),

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
    Inspect(#[from] inspect::Error),
    #[error(transparent)]
    Optimize(#[from] optimize::Error),
    #[error(transparent)]
    Invoke(#[from] invoke::Error),
    #[error(transparent)]
    Events(#[from] events::Error),
    #[error(transparent)]
    Read(#[from] read::Error),
    #[error(transparent)]
    Serve(#[from] serve::Error),
    #[error(transparent)]
    Token(#[from] token::Error),
    #[error(transparent)]
    Gen(#[from] gen::Error),
    #[error(transparent)]
    Deploy(#[from] deploy::Error),
    #[error(transparent)]
    Install(#[from] install::Error),
    #[error(transparent)]
    Xdr(#[from] xdr::Error),
}

async fn run(cmd: Cmd, matches: &mut clap::ArgMatches) -> Result<(), CmdError> {
    match cmd {
        Cmd::Inspect(inspect) => inspect.run()?,
        Cmd::Optimize(opt) => opt.run()?,
        Cmd::Invoke(invoke) => {
            let (_, sub_arg_matches) = matches.remove_subcommand().unwrap();
            invoke.run(&sub_arg_matches).await?;
        }
        Cmd::Events(events) => {
            let (_, sub_arg_matches) = matches.remove_subcommand().unwrap();
            events.run(&sub_arg_matches).await?;
        }
        Cmd::Read(read) => read.run()?,
        Cmd::Serve(serve) => serve.run().await?,
        Cmd::Token(token) => token.run().await?,
        Cmd::Gen(gen) => gen.run()?,
        Cmd::Deploy(deploy) => deploy.run().await?,
        Cmd::Install(install) => install.run().await?,
        Cmd::Xdr(xdr) => xdr.run()?,
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
    let mut saved_matches = matches.clone();
    let root = match Root::from_arg_matches_mut(&mut matches) {
        Ok(s) => s,
        Err(e) => {
            let mut cmd = Root::command();
            e.format(&mut cmd).exit()
        }
    };

    if let Err(e) = run(root.cmd, &mut saved_matches).await {
        eprintln!("error: {e}");
    }
}
