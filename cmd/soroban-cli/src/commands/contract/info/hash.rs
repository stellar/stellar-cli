use std::path::PathBuf;

use clap::Parser;

use crate::{
    commands::global,
    config::{
        self, locator,
        network::{self},
    },
    wasm,
};

#[derive(Parser, Debug, Clone)]
#[command(group(
    clap::ArgGroup::new("source")
        .required(true)
        .args(&["wasm", "contract_id"]),
))]
#[group(skip)]
pub struct Cmd {
    /// Path to a local .wasm file.
    #[arg(long, conflicts_with = "contract_id")]
    pub wasm: Option<PathBuf>,
    /// Contract ID or alias of a deployed contract.
    #[arg(
        long,
        visible_alias = "id",
        env = "STELLAR_CONTRACT_ID",
        conflicts_with = "wasm"
    )]
    pub contract_id: Option<config::UnresolvedContract>,
    #[command(flatten)]
    pub network: network::Args,
    #[command(flatten)]
    pub locator: locator::Args,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Wasm(#[from] wasm::Error),
    #[error(transparent)]
    Network(#[from] network::Error),
    #[error(transparent)]
    Locator(#[from] locator::Error),
}

impl Cmd {
    pub async fn run(&self, _global_args: &global::Args) -> Result<(), Error> {
        let hash = if let Some(path) = &self.wasm {
            wasm::Args { wasm: path.clone() }.hash()?
        } else if let Some(contract_id) = &self.contract_id {
            let network = self.network.get(&self.locator)?;
            let resolved =
                contract_id.resolve_contract_id(&self.locator, &network.network_passphrase)?;
            wasm::fetch_wasm_hash_from_contract(&resolved, &network).await?
        } else {
            unreachable!("clap ArgGroup guarantees one of --wasm or --contract-id is set");
        };

        println!("{hash}");
        Ok(())
    }
}
