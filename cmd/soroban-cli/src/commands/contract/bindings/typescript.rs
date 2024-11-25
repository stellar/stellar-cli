use std::{ffi::OsString, fmt::Debug, path::PathBuf};

use clap::{command, Parser};
use soroban_spec_tools::contract as contract_spec;
use soroban_spec_typescript::{self as typescript, boilerplate::Project};
use stellar_strkey::DecodeError;

use crate::print::Print;
use crate::wasm;
use crate::{
    commands::{contract::fetch, global, NetworkRunnable},
    config::{self, locator, network},
    get_spec::{self, get_remote_contract_spec},
    xdr::{Hash, ScAddress},
};

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    /// Path to optional wasm binary
    #[arg(long)]
    pub wasm: Option<std::path::PathBuf>,
    /// Where to place generated project
    #[arg(long)]
    pub output_dir: PathBuf,
    /// Whether to overwrite output directory if it already exists
    #[arg(long)]
    pub overwrite: bool,
    /// The contract ID/address on the network
    #[arg(long, visible_alias = "id")]
    pub contract_id: String,
    #[command(flatten)]
    pub locator: locator::Args,
    #[command(flatten)]
    pub network: network::Args,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("failed generate TS from file: {0}")]
    GenerateTSFromFile(typescript::GenerateFromFileError),
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("--output-dir cannot be a file: {0:?}")]
    IsFile(PathBuf),

    #[error("--output-dir already exists and you did not specify --overwrite: {0:?}")]
    OutputDirExists(PathBuf),

    #[error("--output-dir filepath not representable as utf-8: {0:?}")]
    NotUtf8(OsString),

    #[error(transparent)]
    Network(#[from] network::Error),

    #[error(transparent)]
    Locator(#[from] locator::Error),
    #[error(transparent)]
    Fetch(#[from] fetch::Error),
    #[error(transparent)]
    Spec(#[from] contract_spec::Error),
    #[error(transparent)]
    Wasm(#[from] wasm::Error),
    #[error("Failed to get file name from path: {0:?}")]
    FailedToGetFileName(PathBuf),
    #[error("cannot parse contract ID {0}: {1}")]
    CannotParseContractId(String, DecodeError),
    #[error(transparent)]
    UtilsError(#[from] get_spec::Error),
    #[error(transparent)]
    Config(#[from] config::Error),
}

#[async_trait::async_trait]
impl NetworkRunnable for Cmd {
    type Error = Error;
    type Result = ();

    async fn run_against_rpc_server(
        &self,
        global_args: Option<&global::Args>,
        config: Option<&config::Args>,
    ) -> Result<(), Error> {
        let print = Print::new(global_args.is_some_and(|a| a.quiet));

        let network = self.network.get(&self.locator).ok().unwrap_or_else(|| {
            network::DEFAULTS
                .get("testnet")
                .expect("no network specified and testnet network not found")
                .into()
        });

        let contract_id = self
            .locator
            .resolve_contract_id(&self.contract_id, &network.network_passphrase)?
            .0;
        let contract_address = ScAddress::Contract(Hash(contract_id));

        let spec = if let Some(wasm) = &self.wasm {
            print.infoln("Loading contract spec from file...");
            let wasm: wasm::Args = wasm.into();
            wasm.parse()?.spec
        } else {
            print.globeln(format!("Downloading contract spec: {contract_address}"));
            get_remote_contract_spec(
                &contract_id,
                &self.locator,
                &self.network,
                global_args,
                config,
            )
            .await
            .map_err(Error::from)?
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
        print.infoln(format!("Network: {}", network.network_passphrase));
        let absolute_path = self.output_dir.canonicalize()?;
        let file_name = absolute_path
            .file_name()
            .ok_or_else(|| Error::FailedToGetFileName(absolute_path.clone()))?;
        let contract_name = &file_name
            .to_str()
            .ok_or_else(|| Error::NotUtf8(file_name.to_os_string()))?;
        print.infoln(format!("Embedding contract address: {contract_address}"));
        p.init(
            contract_name,
            &contract_address.to_string(),
            &network.rpc_url,
            &network.network_passphrase,
            &spec,
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
