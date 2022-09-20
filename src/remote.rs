mod deploy;
mod invoke;

use std::fmt::Debug;

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
pub struct Cmd {
    /// Horizon URL
    #[clap(long, conflicts_with("horizon_url"))]
    horizon_url: Option<String>,
    /// RPC URL
    #[clap(long, conflicts_with("horizon_url"))]
    rpc_url: Option<String>,
    #[clap(subcommand)]
    cmd: SubCmd,
}

pub enum Remote<'a> {
    HorizonUrl(&'a str),
    RpcUrl(&'a str),
}

#[derive(Subcommand, Debug)]
enum SubCmd {
    /// Deploy a WASM file as a contract
    Deploy(deploy::Cmd),
    /// Invoke a contract
    Invoke(invoke::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("no --horizon-url or --rpc-url provided")]
    NoUrl,
    #[error(transparent)]
    Deploy(#[from] deploy::Error),
    #[error(transparent)]
    Invoke(#[from] invoke::Error),
}

impl Cmd {
    pub fn run(&self, matches: &mut clap::ArgMatches) -> Result<(), Error> {
        let remote = if let Some(horizon_url) = &self.horizon_url {
            Remote::HorizonUrl(horizon_url)
        } else if let Some(rpc_url) = &self.rpc_url {
            Remote::RpcUrl(rpc_url)
        } else {
            return Err(Error::NoUrl);
        };
        match &self.cmd {
            SubCmd::Deploy(deploy) => deploy.run(&remote)?,
            SubCmd::Invoke(invoke) => {
                let (_, sub_arg_matches) = matches.remove_subcommand().unwrap();
                invoke.run(&remote, &sub_arg_matches)?;
            }
        };
        Ok(())
    }
}
