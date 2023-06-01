use std::{fmt::Debug, path::PathBuf};

use clap::{command, Parser};
use soroban_spec::gen::ts;

use crate::commands::config::{
    locator,
    network::{self, Network},
};
use crate::wasm;

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    wasm: wasm::Args,

    /// where to place generated project
    #[arg(long)]
    root_dir: PathBuf,

    #[arg(long)]
    contract_name: String,

    #[arg(long, alias = "id")]
    contract_id: String,

    #[command(flatten)]
    locator: locator::Args,

    #[command(flatten)]
    network: network::Args,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("failed generate TS from file: {0}")]
    GenerateTSFromFile(ts::GenerateFromFileError),
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("--root-dir cannot be a file: {0:?}")]
    IsFile(PathBuf),

    #[error(transparent)]
    Network(#[from] network::Error),

    #[error(transparent)]
    Locator(#[from] locator::Error),
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        let spec = self.wasm.parse().unwrap().spec;
        if self.root_dir.is_file() {
            return Err(Error::IsFile(self.root_dir.clone()));
        }
        if self.root_dir.exists() {
            std::fs::remove_dir_all(&self.root_dir)?;
        }
        std::fs::create_dir_all(&self.root_dir)?;
        let p: ts::boilerplate::Project = self.root_dir.clone().try_into()?;
        let Network {
            rpc_url,
            network_passphrase,
        } = self.network.get(&self.locator).unwrap_or(Network {
            rpc_url: "https://horizon-testnet.stellar.org".to_string(),
            network_passphrase: "Test SDF Future Network ; October 2022".to_string(),
        });
        p.init(
            &self.contract_name,
            &self.contract_id,
            &rpc_url,
            &network_passphrase,
            &spec,
        )?;
        Ok(())
    }
}
