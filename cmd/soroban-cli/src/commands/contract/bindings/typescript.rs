use std::{ffi::OsString, fmt::Debug, path::PathBuf};

use clap::{command, Parser};
use soroban_spec_tools::contract as contract_spec;
use soroban_spec_typescript::boilerplate::Project;

use crate::print::Print;
use crate::{
    commands::{contract::info::shared as wasm_or_contract, global, NetworkRunnable},
    config,
};
use soroban_spec_tools::contract::Spec;

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub wasm_or_hash_or_contract_id: wasm_or_contract::Args,
    /// Where to place generated project
    #[arg(long)]
    pub output_dir: PathBuf,
    /// Whether to overwrite output directory if it already exists
    #[arg(long)]
    pub overwrite: bool,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("--output-dir cannot be a file: {0:?}")]
    IsFile(PathBuf),

    #[error("--output-dir already exists and you did not specify --overwrite: {0:?}")]
    OutputDirExists(PathBuf),

    #[error("--output-dir filepath not representable as utf-8: {0:?}")]
    NotUtf8(OsString),

    #[error(transparent)]
    Spec(#[from] contract_spec::Error),
    #[error("Failed to get file name from path: {0:?}")]
    FailedToGetFileName(PathBuf),
    #[error(transparent)]
    WasmOrContract(#[from] wasm_or_contract::Error),
}

#[async_trait::async_trait]
impl NetworkRunnable for Cmd {
    type Error = Error;
    type Result = ();

    async fn run_against_rpc_server(
        &self,
        global_args: Option<&global::Args>,
        _config: Option<&config::Args>,
    ) -> Result<(), Error> {
        let print = Print::new(global_args.is_some_and(|a| a.quiet));

        let (spec, contract_address, network) =
            wasm_or_contract::fetch_wasm(&self.wasm_or_hash_or_contract_id, &print).await?;

        let spec = if let Some(spec) = spec {
            Spec::new(&spec)?
        } else {
            Spec::new(&soroban_sdk::token::StellarAssetSpec::spec_xdr())?
        };

        if self.output_dir.is_file() {
            return Err(Error::IsFile(self.output_dir.clone()));
        }
        if self.output_dir.exists() {
            if self.overwrite {
                std::fs::remove_dir_all(&self.output_dir)?;
            } else {
                return Err(Error::OutputDirExists(self.output_dir.clone()));
            }
        }
        std::fs::create_dir_all(&self.output_dir)?;
        let p: Project = self.output_dir.clone().try_into()?;
        let absolute_path = self.output_dir.canonicalize()?;
        let file_name = absolute_path
            .file_name()
            .ok_or_else(|| Error::FailedToGetFileName(absolute_path.clone()))?;
        let contract_name = &file_name
            .to_str()
            .ok_or_else(|| Error::NotUtf8(file_name.to_os_string()))?;
        if let Some(contract_address) = contract_address.clone() {
            print.infoln(format!("Embedding contract address: {contract_address}"));
        }
        p.init(
            contract_name,
            contract_address.as_deref(),
            network.as_ref().map(|n| n.rpc_url.as_ref()),
            network.as_ref().map(|n| n.network_passphrase.as_ref()),
            &spec.spec,
        )?;
        print.checkln("Generated!");
        print.infoln(format!(
            "Run \"npm install && npm run build\" in {:?} to build the JavaScript NPM package.",
            self.output_dir
        ));
        Ok(())
    }
}

impl Cmd {
    pub async fn run(&self) -> Result<(), Error> {
        self.run_against_rpc_server(None, None).await
    }
}
