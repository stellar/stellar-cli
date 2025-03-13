use std::fmt::Debug;

use crate::commands::global;

pub mod build;
pub mod env_meta;
pub mod interface;
pub mod meta;
pub mod shared;

#[derive(Debug, clap::Subcommand)]
pub enum Cmd {
    /// Output the interface of a contract.
    ///
    /// A contract's interface describes the functions, parameters, and
    /// types that the contract makes accessible to be called.
    ///
    /// The data outputted by this command is a stream of `SCSpecEntry` XDR values.
    /// See the type definitions in [stellar-xdr](https://github.com/stellar/stellar-xdr).
    /// [See also XDR data format](https://developers.stellar.org/docs/learn/encyclopedia/data-format/xdr).
    ///
    /// Outputs no data when no data is present in the contract.
    Interface(interface::Cmd),

    /// Output the metadata stored in a contract.
    ///
    /// A contract's meta is a series of key-value pairs that the contract
    /// developer can set with any values to provided metadata about the
    /// contract. The meta also contains some information like the version
    /// of Rust SDK, and Rust compiler version.
    ///
    /// The data outputted by this command is a stream of `SCMetaEntry` XDR values.
    /// See the type definitions in [stellar-xdr](https://github.com/stellar/stellar-xdr).
    /// [See also XDR data format](https://developers.stellar.org/docs/learn/encyclopedia/data-format/xdr).
    ///
    /// Outputs no data when no data is present in the contract.
    Meta(meta::Cmd),

    /// Output the env required metadata stored in a contract.
    ///
    /// Env-meta is information stored in all contracts, in the
    /// `contractenvmetav0` WASM custom section, about the environment
    /// that the contract was built for. Env-meta allows the Soroban Env
    /// to know whether the contract is compatibility with the network in
    /// its current configuration.
    ///
    /// The data outputted by this command is a stream of `SCEnvMetaEntry` XDR values.
    /// See the type definitions in [stellar-xdr](https://github.com/stellar/stellar-xdr).
    /// [See also XDR data format](https://developers.stellar.org/docs/learn/encyclopedia/data-format/xdr).
    ///
    /// Outputs no data when no data is present in the contract.
    EnvMeta(env_meta::Cmd),

    /// Output the contract build information, if available.
    ///
    /// If the contract has a meta entry like `source_repo=github:user/repo`, this command will try
    /// to fetch the attestation information for the WASM file.
    Build(build::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Interface(#[from] interface::Error),

    #[error(transparent)]
    Meta(#[from] meta::Error),

    #[error(transparent)]
    EnvMeta(#[from] env_meta::Error),

    #[error(transparent)]
    Build(#[from] build::Error),
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        match &self {
            Cmd::Interface(interface) => interface.run(global_args).await?,
            Cmd::Meta(meta) => meta.run(global_args).await?,
            Cmd::EnvMeta(env_meta) => env_meta.run(global_args).await?,
            Cmd::Build(build) => build.run(global_args).await?,
        };

        Ok(())
    }
}
