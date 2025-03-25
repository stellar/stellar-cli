pub mod arg_parsing;
pub mod deploy;
pub mod extend;
pub mod fetch;
pub mod id;
pub mod inspect;
pub mod invoke;
pub mod optimize;
pub mod policy;
pub mod read;
pub mod restore;
pub mod upload;

use crate::{commands::global, print::Print};
use clap::Subcommand;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, clap::ValueEnum)]
pub enum Durability {
    /// Persistent
    Persistent,
    /// Temporary
    Temporary,
}

impl From<&Durability> for crate::xdr::ContractDataDurability {
    fn from(d: &Durability) -> Self {
        match d {
            Durability::Persistent => crate::xdr::ContractDataDurability::Persistent,
            Durability::Temporary => crate::xdr::ContractDataDurability::Temporary,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, clap::ValueEnum)]
pub enum SpecOutput {
    /// XDR of array of contract spec entries
    XdrBase64,
    /// Array of xdr of contract spec entries
    XdrBase64Array,
    /// Pretty print of contract spec entries
    Docs,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Cmd {
    /// Deploy a contract
    Deploy(deploy::Cmd),
    /// Extend a contract's TTL
    Extend(extend::Cmd),
    /// Fetch a contract's WASM
    Fetch(fetch::Cmd),
    /// Inspect a contract's WASM
    Inspect(inspect::Cmd),
    /// Invoke a contract function
    Invoke(invoke::Cmd),
    /// Optimize a contract's WASM
    Optimize(optimize::Cmd),
    /// Generate a policy contract
    Policy(policy::Cmd),
    /// Read a contract's persistent data
    Read(read::Cmd),
    /// Restore a contract's persistent data
    Restore(restore::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Deploy(#[from] deploy::Error),
    #[error(transparent)]
    Extend(#[from] extend::Error),
    #[error(transparent)]
    Fetch(#[from] fetch::Error),
    #[error(transparent)]
    Inspect(#[from] inspect::Error),
    #[error(transparent)]
    Invoke(#[from] invoke::Error),
    #[error(transparent)]
    Optimize(#[from] optimize::Error),
    #[error(transparent)]
    Policy(#[from] policy::Error),
    #[error(transparent)]
    Read(#[from] read::Error),
    #[error(transparent)]
    Restore(#[from] restore::Error),
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let _print = Print::new(global_args.quiet);
        match self {
            Cmd::Deploy(deploy) => deploy.run(global_args).await.map_err(Error::Deploy),
            Cmd::Extend(extend) => extend.run().await.map_err(Error::Extend),
            Cmd::Fetch(fetch) => fetch.run().await.map_err(Error::Fetch),
            Cmd::Inspect(inspect) => inspect.run(global_args).map_err(Error::Inspect),
            Cmd::Invoke(invoke) => invoke.run(global_args).await.map_err(Error::Invoke),
            Cmd::Optimize(optimize) => optimize.run().map_err(Error::Optimize),
            Cmd::Policy(policy) => policy.run(global_args).await.map_err(Error::Policy),
            Cmd::Read(read) => read.run().await.map_err(Error::Read),
            Cmd::Restore(restore) => restore.run().await.map_err(Error::Restore),
        }
    }
}
