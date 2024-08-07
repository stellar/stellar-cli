use std::fmt::Debug;

use crate::commands::contract::info::meta::Error::{NoMetaPresent, NoSACMeta};
use crate::commands::contract::info::shared;
use crate::commands::contract::info::shared::fetch_wasm;
use clap::{command, Parser};
use soroban_spec_tools::contract;
use soroban_spec_tools::contract::Spec;
use stellar_xdr::curr::{ScMetaEntry, ScMetaV0};

// use crate::commands::contract::info::shared::fetch_wasm;
use crate::commands::contract::InfoOutput;

#[derive(Parser, Debug, Clone)]
pub struct Cmd {
    #[command(flatten)]
    pub common: shared::Args,
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

        if bytes.is_none() {
            return Err(NoSACMeta());
        }
        let spec = Spec::new(&bytes.unwrap())?;

        if spec.meta_base64.is_none() {
            return Err(NoMetaPresent());
        }

        let res = match self.common.output {
            InfoOutput::XdrBase64 => spec.meta_base64.unwrap(),
            InfoOutput::Json => serde_json::to_string(&spec.meta)?,
            InfoOutput::JsonFormatted => serde_json::to_string_pretty(&spec.meta)?,
            InfoOutput::Pretty => {
                let mut meta_str = "Contract meta:\n".to_string();

                for meta_entry in &spec.meta {
                    match meta_entry {
                        ScMetaEntry::ScMetaV0(ScMetaV0 { key, val }) => {
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
