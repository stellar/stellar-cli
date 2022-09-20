mod deploy;
mod invoke;
mod read;

use std::fmt::Debug;

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
pub struct Cmd {
    /// File to persist ledger state
    #[clap(long, parse(from_os_str), default_value(".soroban/ledger.json"))]
    ledger_file: std::path::PathBuf,
    #[clap(subcommand)]
    cmd: SubCmd,
}

#[derive(Subcommand, Debug)]
enum SubCmd {
    /// Invoke a contract function in a WASM file
    Invoke(invoke::Cmd),
    /// Print the current value of a contract-data ledger entry
    Read(read::Cmd),
    /// Deploy a WASM file as a contract
    Deploy(deploy::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Invoke(#[from] invoke::Error),
    #[error(transparent)]
    Read(#[from] read::Error),
    #[error(transparent)]
    Deploy(#[from] deploy::Error),
}

impl Cmd {
    pub fn run(&self, matches: &mut clap::ArgMatches) -> Result<(), Error> {
        match &self.cmd {
            SubCmd::Invoke(invoke) => {
                let (_, sub_arg_matches) = matches.remove_subcommand().unwrap();
                invoke.run(&self.ledger_file, &sub_arg_matches)?;
            }
            SubCmd::Read(read) => read.run(&self.ledger_file)?,
            SubCmd::Deploy(deploy) => deploy.run(&self.ledger_file)?,
        };
        Ok(())
    }
}
