use clap::{AppSettings, CommandFactory, FromArgMatches, Parser, Subcommand};

mod completion;
mod contractspec;
mod deploy;
mod error;
mod gen;
mod inspect;
mod invoke;
mod jsonrpc;
mod network;
mod read;
mod serve;
mod snapshot;
mod strval;
mod utils;
mod version;

#[derive(Parser, Debug)]
#[clap(
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
    /// Print the current value of a contract-data ledger entry
    Read(read::Cmd),
    /// Run a local webserver for web app development and testing
    Serve(serve::Cmd),
    /// Deploy a WASM file as a contract
    Deploy(deploy::Cmd),
    /// Generate code client bindings for a contract
    Gen(gen::Cmd),

    /// Print version information
    Version(version::Cmd),
    /// Print shell completion code for the specified shell.
    #[clap(long_about = completion::LONG_ABOUT)]
    Completion(completion::Cmd),
}

async fn run(cmd: Cmd, matches: &mut clap::ArgMatches) -> Result<(), error::Cmd> {
    match cmd {
        Cmd::Inspect(inspect) => inspect.run()?,
        Cmd::Invoke(invoke) => {
            let (_, sub_arg_matches) = matches.remove_subcommand().unwrap();
            invoke.run(&sub_arg_matches)?;
        }
        Cmd::Read(read) => read.run()?,
        Cmd::Serve(serve) => serve.run().await?,
        Cmd::Gen(gen) => gen.run()?,
        Cmd::Deploy(deploy) => deploy.run()?,
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
        eprintln!("error: {}", e);
    }
}
