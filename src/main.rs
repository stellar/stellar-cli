use clap::{Parser, Subcommand};
use thiserror::Error;

mod strval;

mod inspect;
use inspect::Inspect;

mod invoke;
use invoke::Invoke;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Root {
    #[clap(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    Inspect(Inspect),
    Invoke(Invoke),
}

#[derive(Error, Debug)]
enum CmdError {
    #[error("inspect")]
    Inspect(#[from] inspect::Error),
    #[error("invoke")]
    Invoke(#[from] invoke::Error),
}

fn run(cmd: Cmd) -> Result<(), CmdError> {
    match cmd {
        Cmd::Inspect(inspect) => inspect.run()?,
        Cmd::Invoke(invoke) => invoke.run()?,
    };
    Ok(())
}

fn main() {
    let root = Root::parse();
    if let Err(e) = run(root.cmd) {
        println!("error: {:?}", e);
    }
}
