use clap::{ArgMatches, Subcommand};

pub mod bindings;
pub mod deploy;
pub mod inspect;
pub mod invoke;
pub mod optimize;
pub mod read;

#[derive(Debug, Subcommand)]
pub enum SubCmd {
    /// Generate code client bindings for a contract
    Bindings(bindings::Cmd),

    /// Deploy a contract
    Deploy(deploy::Cmd),

    /// Inspect a WASM file listing contract functions, meta, etc
    Inspect(inspect::Cmd),

    /// Invoke a contract function
    Invoke(invoke::Cmd),

    /// Optimize a WASM file
    Optimize(optimize::Cmd),

    /// Print the current value of a contract-data ledger entry
    Read(read::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Bindings(#[from] bindings::Error),

    #[error(transparent)]
    Deploy(#[from] deploy::Error),

    #[error(transparent)]
    Inspect(#[from] inspect::Error),

    #[error(transparent)]
    Invoke(#[from] invoke::Error),

    #[error(transparent)]
    Optimize(#[from] optimize::Error),

    #[error(transparent)]
    Read(#[from] read::Error),
}

impl SubCmd {
    pub async fn run(&self, sub_arg_matches: &ArgMatches) -> Result<(), Error> {
        match &self {
            SubCmd::Bindings(bindings) => bindings.run()?,
            SubCmd::Deploy(deploy) => deploy.run().await?,
            SubCmd::Inspect(inspect) => inspect.run()?,
            SubCmd::Invoke(invoke) => {
                invoke.run(sub_arg_matches).await?;
            }
            SubCmd::Optimize(optimize) => optimize.run()?,
            SubCmd::Read(read) => read.run()?,
        }
        Ok(())
    }
}
