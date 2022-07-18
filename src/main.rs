use clap::{AppSettings, Parser, Subcommand};
use thiserror::Error;

mod strval;

mod deploy;
mod inspect;
mod invoke;
mod snapshot;
mod version;

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
    /// Writes a contractID and WASM contract directly to storage
    Deploy(deploy::Cmd),
    /// Print version information
    Version(version::Cmd),
}

#[derive(Error, Debug)]
enum CmdError {
    #[error("inspect")]
    Inspect(#[from] inspect::Error),
    #[error("invoke")]
    Invoke(#[from] invoke::Error),
    #[error("deploy")]
    Deploy(#[from] deploy::Error),
}

fn run(cmd: Cmd) -> Result<(), CmdError> {
    match cmd {
        Cmd::Inspect(inspect) => inspect.run()?,
        Cmd::Invoke(invoke) => invoke.run()?,
        Cmd::Deploy(deploy) => deploy.run()?,
        Cmd::Version(version) => version.run(),
    };
    Ok(())
}

fn main() {
    let root = Root::parse();
    if let Err(e) = run(root.cmd) {
        eprintln!("error: {:?}", e);
    }
}
