pub mod bindings;
pub mod deploy;
pub mod inspect;
pub mod install;
pub mod invoke;
pub mod optimize;
pub mod read;

#[derive(Debug, clap::Subcommand)]
pub enum Cmd {
    /// Generate code client bindings for a contract
    Bindings(bindings::Cmd),

    /// Deploy a contract
    Deploy(deploy::Cmd),

    /// Inspect a WASM file listing contract functions, meta, etc
    Inspect(inspect::Cmd),

    /// Install a WASM file to the ledger without creating a contract instance
    Install(install::Cmd),

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
    Install(#[from] install::Error),

    #[error(transparent)]
    Invoke(#[from] invoke::Error),

    #[error(transparent)]
    Optimize(#[from] optimize::Error),

    #[error(transparent)]
    Read(#[from] read::Error),
}

impl Cmd {
    pub async fn run(&self) -> Result<(), Error> {
        match &self {
            Cmd::Bindings(bindings) => bindings.run()?,
            Cmd::Deploy(deploy) => deploy.run().await?,
            Cmd::Inspect(inspect) => inspect.run()?,
            Cmd::Install(install) => install.run().await?,
            Cmd::Invoke(invoke) => invoke.run().await?,
            Cmd::Optimize(optimize) => optimize.run()?,
            Cmd::Read(read) => read.run()?,
        }
        Ok(())
    }
}
