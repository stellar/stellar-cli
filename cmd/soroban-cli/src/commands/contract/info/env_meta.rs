use std::{fmt::Debug, path::PathBuf};

use clap::{command, Parser};

use crate::commands::contract::InfoOutput;

#[derive(Parser, Debug, Clone)]
#[command(group(
    clap::ArgGroup::new("src")
    .required(true)
    .args(& ["wasm", "wasm_hash", "contract_id"]),
))]
#[group(skip)]
pub struct Cmd {
    /// Wasm file to extract the meta from
    #[arg(
        long,
        conflicts_with = "wasm_hash",
        conflicts_with = "contract_id",
        group = "src"
    )]
    pub wasm: Option<PathBuf>,
    /// Wasm hash to get the meta for
    #[arg(long = "wasm-hash", group = "src")]
    pub wasm_hash: Option<String>,
    /// Contract ID to get the meta for
    #[arg(long = "id", env = "STELLAR_CONTRACT_ID", group = "src")]
    pub contract_id: Option<String>,
    /// Format of the output
    #[arg(long, default_value = "xdr-base64")]
    output: InfoOutput,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {}

impl Cmd {
    pub async fn run(&self) -> Result<String, Error> {
        Ok("env_meta".to_string()) // TODO
    }
}
