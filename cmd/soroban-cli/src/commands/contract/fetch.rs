use std::convert::Infallible;

use std::io::Write;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::{fmt::Debug, fs, io};

use clap::Parser;

use crate::{
    config::{
        self, locator,
        network::{self, Network},
    },
    wasm, xdr, Pwd,
};

#[derive(Parser, Debug, Default, Clone)]
#[allow(clippy::struct_excessive_bools)]
#[group(skip)]
pub struct Cmd {
    /// Contract ID to fetch
    #[arg(long = "id", env = "STELLAR_CONTRACT_ID")]
    pub contract_id: Option<config::UnresolvedContract>,
    /// Wasm to fetch
    #[arg(long = "wasm-hash", conflicts_with = "contract_id")]
    pub wasm_hash: Option<String>,
    /// Where to write output otherwise stdout is used
    #[arg(long, short = 'o')]
    pub out_file: Option<std::path::PathBuf>,
    #[command(flatten)]
    pub locator: locator::Args,
    #[command(flatten)]
    pub network: network::Args,
}

impl FromStr for Cmd {
    type Err = clap::error::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use clap::{CommandFactory, FromArgMatches};
        Self::from_arg_matches_mut(&mut Self::command().get_matches_from(s.split_whitespace()))
    }
}

impl Pwd for Cmd {
    fn set_pwd(&mut self, pwd: &Path) {
        self.locator.set_pwd(pwd);
    }
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Config(#[from] config::Error),
    #[error(transparent)]
    Locator(#[from] locator::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("reading file {0:?}: {1}")]
    CannotWriteContractFile(PathBuf, io::Error),
    #[error(transparent)]
    Network(#[from] network::Error),
    #[error("cannot create contract directory for {0:?}")]
    CannotCreateContractDir(PathBuf),
    #[error(transparent)]
    Wasm(#[from] wasm::Error),
    #[error("wasm hash is invalid {0:?}")]
    InvalidWasmHash(String),
    #[error("must provide one of --wasm-hash, or --id")]
    MissingArg,
}

impl From<Infallible> for Error {
    fn from(_: Infallible) -> Self {
        unreachable!()
    }
}

impl Cmd {
    pub async fn run(&self) -> Result<(), Error> {
        let bytes = self.get_bytes().await?;
        if let Some(out_file) = &self.out_file {
            if let Some(parent) = out_file.parent() {
                if !parent.exists() {
                    fs::create_dir_all(parent)
                        .map_err(|_| Error::CannotCreateContractDir(out_file.clone()))?;
                }
            }
            fs::write(out_file, bytes)
                .map_err(|io| Error::CannotWriteContractFile(out_file.clone(), io))
        } else {
            let stdout = std::io::stdout();
            let mut handle = stdout.lock();
            handle.write_all(&bytes)?;
            handle.flush()?;
            Ok(())
        }
    }

    pub async fn get_bytes(&self) -> Result<Vec<u8>, Error> {
        self.execute(&config::Args {
            locator: self.locator.clone(),
            network: self.network.clone(),
            source_account: Default::default(),
            sign_with: Default::default(),
            fee: None,
            inclusion_fee: None,
        })
        .await
    }

    pub fn network(&self) -> Result<Network, Error> {
        Ok(self.network.get(&self.locator)?)
    }

    pub async fn execute(&self, config: &config::Args) -> Result<Vec<u8>, Error> {
        let network = config.get_network()?;
        if let Some(contract_id) = &self.contract_id {
            Ok(wasm::fetch_from_contract(
                &contract_id.resolve_contract_id(&self.locator, &network.network_passphrase)?,
                &network,
            )
            .await?)
        } else if let Some(wasm_hash) = &self.wasm_hash {
            let hash = hex::decode(wasm_hash)
                .map_err(|_| Error::InvalidWasmHash(wasm_hash.clone()))?
                .try_into()
                .map_err(|_| Error::InvalidWasmHash(wasm_hash.clone()))?;
            let hash = xdr::Hash(hash);
            Ok(wasm::fetch_from_wasm_hash(hash, &network).await?)
        } else {
            Err(Error::MissingArg)
        }
    }
}
