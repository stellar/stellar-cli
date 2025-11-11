use crate::{commands::global, print::Print, utils::deprecate_message};

use super::{deploy, id};

#[derive(Debug, clap::Subcommand)]
pub enum Cmd {
    /// Get Id of builtin Soroban Asset Contract. Deprecated, use `stellar contract id asset` instead
    Id(id::asset::Cmd),
    /// Deploy builtin Soroban Asset Contract
    Deploy(deploy::asset::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Id(#[from] id::asset::Error),
    #[error(transparent)]
    Deploy(#[from] deploy::asset::Error),
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let print = Print::new(global_args.quiet);

        match &self {
            Cmd::Id(id) => {
                deprecate_message(
                    print,
                    "stellar contract asset id",
                    "Use `stellar contract id asset` instead.",
                );
                id.run()?;
            }
            Cmd::Deploy(asset) => asset.run(global_args).await?,
        }
        Ok(())
    }
}
