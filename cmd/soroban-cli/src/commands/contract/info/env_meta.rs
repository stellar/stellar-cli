use std::fmt::Debug;

use clap::{command, Parser};
use soroban_sdk::xdr::ScEnvMetaEntryInterfaceVersion;
use stellar_xdr::curr::ScEnvMetaEntry;

use soroban_spec_tools::contract;
use soroban_spec_tools::contract::Spec;

use crate::commands::contract::info::env_meta::Error::{NoEnvMetaPresent, NoSACEnvMeta};
use crate::commands::contract::info::shared;
use crate::commands::contract::info::shared::{fetch_wasm, MetasInfoOutput};

#[derive(Parser, Debug, Clone)]
pub struct Cmd {
    #[command(flatten)]
    pub common: shared::Args,
    /// Format of the output
    #[arg(long, default_value = "text")]
    pub output: MetasInfoOutput,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Wasm(#[from] shared::Error),
    #[error(transparent)]
    Spec(#[from] contract::Error),
    #[error("Stellar asset contract doesn't contain meta information")]
    NoSACEnvMeta(),
    #[error("no meta present in provided WASM file")]
    NoEnvMetaPresent(),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

impl Cmd {
    pub async fn run(&self) -> Result<String, Error> {
        let bytes = fetch_wasm(&self.common).await?;

        let Some(bytes) = bytes else {
            return Err(NoSACEnvMeta());
        };
        let spec = Spec::new(&bytes)?;

        let Some(env_meta_base64) = spec.env_meta_base64 else {
            return Err(NoEnvMetaPresent());
        };

        let res = match self.output {
            MetasInfoOutput::XdrBase64 => env_meta_base64,
            MetasInfoOutput::Json => serde_json::to_string(&spec.env_meta)?,
            MetasInfoOutput::JsonFormatted => serde_json::to_string_pretty(&spec.env_meta)?,
            MetasInfoOutput::Text => {
                let mut meta_str = "Contract env-meta:\n".to_string();
                for env_meta_entry in &spec.env_meta {
                    match env_meta_entry {
                        ScEnvMetaEntry::ScEnvMetaKindInterfaceVersion(
                            ScEnvMetaEntryInterfaceVersion {
                                protocol,
                                pre_release,
                            },
                        ) => {
                            meta_str.push_str(&format!(" • Protocol: v{protocol}\n"));
                            if pre_release != &0 {
                                meta_str.push_str(&format!(" • Pre-release: v{pre_release}\n"));
                            }
                        }
                    }
                }
                meta_str
            }
        };

        Ok(res)
    }
}
