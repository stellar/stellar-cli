use async_trait::async_trait;
use clap::Subcommand;
use crate::commands::global;
use clap::Parser;

pub mod asset;
pub mod utils;
pub mod wasm;

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(subcommand)]
    pub cmd: SubCmd,
}

#[derive(Parser, Debug, Clone)]
pub enum SubCmd {
    /// Deploy a WASM smart contract
    #[command(name = "wasm")]
    Wasm(wasm::Cmd),
    /// Deploy an asset contract
    #[command(name = "asset")]
    Asset(asset::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Asset(#[from] asset::Error),
    #[error(transparent)]
    Wasm(#[from] wasm::Error),
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        match &self.cmd {
            SubCmd::Wasm(wasm) => wasm.run(global_args).await.map_err(Error::Wasm),
            SubCmd::Asset(asset) => asset.run(global_args).await.map_err(Error::Asset),
        }
    }
}
