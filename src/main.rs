use clap::{AppSettings, Parser, Subcommand, CommandFactory};
use thiserror::Error;

mod contractspec;
mod deploy;
mod inspect;
mod invoke;
mod jsonrpc;
mod serve;
mod snapshot;
mod strval;
mod utils;
mod version;
mod completion;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None, disable_help_subcommand = true, disable_version_flag = true)]
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
    /// Run a local webserver for web app development and testing
    Serve(serve::Cmd),
    /// Deploy a WASM file as a contract
    Deploy(deploy::Cmd),
    /// Print version information
    Version(version::Cmd),
    /// Print shell completion code for the specified shell.
    #[clap(long_about = completion::LONG_ABOUT)]
    Completion(completion::Cmd)
}

#[derive(Error, Debug)]
enum CmdError {
    #[error("inspect")]
    Inspect(#[from] inspect::Error),
    #[error("invoke")]
    Invoke(#[from] invoke::Error),
    #[error("serve")]
    Serve(#[from] serve::Error),
    #[error("deploy")]
    Deploy(#[from] deploy::Error),
}

async fn run(cmd: Cmd) -> Result<(), CmdError> {
    match cmd {
        Cmd::Inspect(inspect) => inspect.run()?,
        Cmd::Invoke(invoke) => invoke.run()?,
        Cmd::Serve(serve) => serve.run().await?,
        Cmd::Deploy(deploy) => deploy.run()?,
        Cmd::Version(version) => version.run(),
        Cmd::Completion(completion) => completion.run(&mut Root::command()),
    };
    Ok(())
}

#[tokio::main]
async fn main() {
    let root = Root::parse();
    if let Err(e) = run(root.cmd).await {
        eprintln!("error: {:?}", e);
    }
}
