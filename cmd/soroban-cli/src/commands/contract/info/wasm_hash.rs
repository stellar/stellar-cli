use std::fmt::Debug;

use crate::commands::contract::info::shared::{self, fetch, Fetched};
use crate::commands::global;
use crate::print::Print;
use clap::{command, Parser};

#[derive(Parser, Debug, Clone)]
pub struct Cmd {
    #[command(flatten)]
    pub common: shared::Args,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Wasm(#[from] shared::Error),
    #[error("cannot get wasm hash from stellar asset contract")]
    StellarAssetContract,
    #[error("failed to calculate wasm hash from local file")]
    HashCalculation(#[from] crate::xdr::Error),
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let print = Print::new(global_args.quiet);
        let Fetched { contract, .. } = fetch(&self.common, &print).await?;

        let wasm_hash = match contract {
            shared::Contract::Wasm { wasm_bytes } => {
                // Calculate hash from wasm bytes
                hex::encode(crate::utils::contract_hash(&wasm_bytes)?)
            }
            shared::Contract::StellarAssetContract => {
                return Err(Error::StellarAssetContract);
            }
        };

        println!("{wasm_hash}");
        Ok(())
    }
}
