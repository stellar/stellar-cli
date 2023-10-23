pub mod bindings;
pub mod build;
pub mod deploy;
pub mod extend;
pub mod fetch;
pub mod inspect;
pub mod install;
pub mod invoke;
pub mod optimize;
pub mod read;
pub mod restore;

use crate::commands::global;

#[derive(Debug, clap::Subcommand)]
pub enum Cmd {
    /// Generate code client bindings for a contract
    #[command(subcommand)]
    Bindings(bindings::Cmd),

    Build(build::Cmd),

    /// Extend the time to live ledger of a contract-data ledger entry.
    ///
    /// If no keys are specified the contract itself is extended.
    Extend(extend::Cmd),

    /// Deploy a contract
    Deploy(deploy::Cmd),

    /// Fetch a contract's Wasm binary
    Fetch(fetch::Cmd),

    /// Inspect a WASM file listing contract functions, meta, etc
    Inspect(inspect::Cmd),

    /// Install a WASM file to the ledger without creating a contract instance
    Install(install::Cmd),

    /// Invoke a contract function
    ///
    /// Generates an "implicit CLI" for the specified contract on-the-fly using the contract's
    /// schema, which gets embedded into every Soroban contract. The "slop" in this command,
    /// everything after the `--`, gets passed to this implicit CLI. Get in-depth help for a given
    /// contract:
    ///
    ///     soroban contract invoke ... -- --help
    Invoke(invoke::Cmd),

    /// Optimize a WASM file
    Optimize(optimize::Cmd),

    /// Print the current value of a contract-data ledger entry
    Read(read::Cmd),

    /// Restore an evicted value for a contract-data legder entry.
    ///
    /// If no keys are specificed the contract itself is restored.
    Restore(restore::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Bindings(#[from] bindings::Error),

    #[error(transparent)]
    Build(#[from] build::Error),

    #[error(transparent)]
    Extend(#[from] extend::Error),

    #[error(transparent)]
    Deploy(#[from] deploy::Error),

    #[error(transparent)]
    Fetch(#[from] fetch::Error),

    #[error(transparent)]
    Inspect(#[from] inspect::Error),

    #[error(transparent)]
    Install(#[from] install::Error),

    #[error(transparent)]
    Invoke(#[from] invoke::Error),

    #[error(transparent)]
    Optimize(#[from] optimize::Error),

    #[error(transparent)]
    Read(#[from] read::Error),

    #[error(transparent)]
    Restore(#[from] restore::Error),
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        match &self {
            Cmd::Bindings(bindings) => bindings.run().await?,
            Cmd::Build(build) => build.run()?,
            Cmd::Extend(extend) => extend.run().await?,
            Cmd::Deploy(deploy) => deploy.run().await?,
            Cmd::Inspect(inspect) => inspect.run()?,
            Cmd::Install(install) => install.run().await?,
            Cmd::Invoke(invoke) => invoke.run(global_args).await?,
            Cmd::Optimize(optimize) => optimize.run()?,
            Cmd::Fetch(fetch) => fetch.run().await?,
            Cmd::Read(read) => read.run().await?,
            Cmd::Restore(restore) => restore.run().await?,
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, clap::ValueEnum)]
pub enum Durability {
    /// Persistent
    Persistent,
    /// Temporary
    Temporary,
}

impl From<&Durability> for soroban_env_host::xdr::ContractDataDurability {
    fn from(d: &Durability) -> Self {
        match d {
            Durability::Persistent => soroban_env_host::xdr::ContractDataDurability::Persistent,
            Durability::Temporary => soroban_env_host::xdr::ContractDataDurability::Temporary,
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
