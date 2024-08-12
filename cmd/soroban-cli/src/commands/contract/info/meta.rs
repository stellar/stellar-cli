use std::fmt::Debug;

use crate::commands::contract::info::meta::Error::{NoMetaPresent, NoSACMeta};
use crate::commands::contract::info::shared;
use crate::commands::contract::info::shared::{fetch_wasm, MetasInfoOutput};
use clap::{command, Parser};
use soroban_spec_tools::contract;
use soroban_spec_tools::contract::Spec;
use stellar_xdr::curr::{ScMetaEntry, ScMetaV0};

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
    NoSACMeta(),
    #[error("no meta present in provided WASM file")]
    NoMetaPresent(),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

impl Cmd {
    pub async fn run(&self) -> Result<String, Error> {
        let bytes = fetch_wasm(&self.common).await?;

        let Some(bytes) = bytes else {
            return Err(NoSACMeta());
        };
        let spec = Spec::new(&bytes)?;

        let Some(meta_base64) = spec.meta_base64 else {
            return Err(NoMetaPresent());
        };

        let res = match self.output {
            MetasInfoOutput::XdrBase64 => meta_base64,
            MetasInfoOutput::Json => serde_json::to_string(&spec.meta)?,
            MetasInfoOutput::JsonFormatted => serde_json::to_string_pretty(&spec.meta)?,
            MetasInfoOutput::Text => {
                let mut meta_str = "Contract meta:\n".to_string();

                for meta_entry in &spec.meta {
                    match meta_entry {
                        ScMetaEntry::ScMetaV0(ScMetaV0 { key, val }) => {
                            let key = key.to_string();
                            let val = match key.as_str() {
                                "rsver" => format!("{val} (Rust version)"),
                                "rssdkver" => {
                                    format!("{val} (Soroban SDK version and it's commit hash)")
                                }
                                _ => val.to_string(),
                            };
                            meta_str.push_str(&format!(" • {key}: {val}\n"));
                        }
                    }
                }

                meta_str
            }
        };

        Ok(res)
    }
}
