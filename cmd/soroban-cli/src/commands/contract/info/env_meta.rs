use std::fmt::Debug;

use crate::commands::contract::info::env_meta::Error::{NoEnvMetaPresent, NoSACEnvMeta};
use crate::commands::contract::info::shared;
use crate::commands::contract::info::shared::fetch_wasm;
use clap::{command, Parser};
use soroban_spec_tools::contract;
use soroban_spec_tools::contract::Spec;

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
    NoSACEnvMeta(),
    #[error("no meta present in provided WASM file")]
    NoEnvMetaPresent(),
}

impl Cmd {
    pub async fn run(&self) -> Result<String, Error> {
        let bytes = fetch_wasm(&self.common).await?;

        if bytes.is_none() {
            return Err(NoSACEnvMeta());
        }
        let spec = Spec::new(&bytes.unwrap())?;

        if spec.env_meta_base64.is_none() {
            return Err(NoEnvMetaPresent());
        }

        let res = match self.common.output {
            InfoOutput::XdrBase64 => spec.env_meta_base64.unwrap(),
            InfoOutput::Json => {
                unreachable!("TODO")
            }
            InfoOutput::JsonFormatted => {
                unreachable!("TODO")
            }
        };

        Ok(res)
    }
}
