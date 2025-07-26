use std::{ffi::OsString, fmt::Debug, path::PathBuf};

use clap::{command, Parser};
use soroban_spec_tools::contract as spec_tools;
use soroban_spec_typescript::mcp_server::{McpServerGenerator, Error as McpError};

use crate::print::Print;
use crate::commands::{contract::info::shared as contract_spec, global};
use soroban_spec_tools::contract::Spec;

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub wasm_or_hash_or_contract_id: contract_spec::Args,
    /// Where to place generated project
    #[arg(long)]
    pub output_dir: PathBuf,
    /// Whether to overwrite output directory if it already exists
    #[arg(long)]
    pub overwrite: bool,
    /// Name for the MCP server
    #[arg(long)]
    pub name: String,
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
    Spec(#[from] spec_tools::Error),

    #[error("Failed to get file name from path: {0:?}")]
    FailedToGetFileName(PathBuf),

    #[error(transparent)]
    WasmOrContract(#[from] contract_spec::Error),

    #[error(transparent)]
    Xdr(#[from] crate::xdr::Error),

    #[error(transparent)]
    McpError(#[from] McpError),
}

impl Cmd {
    pub async fn run(&self, global_args: Option<&global::Args>) -> Result<(), Error> {
        let print = Print::new(global_args.is_some_and(|a| a.quiet));

        let contract_spec::Fetched { contract, source } =
            contract_spec::fetch(&self.wasm_or_hash_or_contract_id, &print).await?;

        let contract_id = match source {
            contract_spec::Source::Contract { resolved_address, .. } => resolved_address,
            _ => return Err(Error::WasmOrContract(contract_spec::Error::MissingArg)),
        };

        let (spec, is_sac) = match contract {
            contract_spec::Contract::Wasm { wasm_bytes } => (Spec::new(&wasm_bytes)?.spec, false),
            contract_spec::Contract::StellarAssetContract => (
                soroban_spec::read::parse_raw(&soroban_sdk::token::StellarAssetSpec::spec_xdr())?,
                true
            ),
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

        // Generate MCP server code
        let mut generator = McpServerGenerator::new(is_sac);
        generator.generate(
            &self.output_dir,
            &self.name,
            &spec,
            &contract_id,
        )?;

        Ok(())
    }
} 