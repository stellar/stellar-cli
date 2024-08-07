use std::fmt::Debug;

use crate::commands::contract::info::interface::Error::NoInterfacePresent;
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
    #[error("no interface present in provided WASM file")]
    NoInterfacePresent(),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

impl Cmd {
    pub async fn run(&self) -> Result<String, Error> {
        let bytes = fetch_wasm(&self.common).await?;

        let (base64, spec) = if bytes.is_none() {
            Spec::spec_to_base64(&soroban_sdk::token::StellarAssetSpec::spec_xdr())?
        } else {
            let spec = Spec::new(&bytes.unwrap())?;

            if spec.env_meta_base64.is_none() {
                return Err(NoInterfacePresent());
            }

            (spec.spec_base64.unwrap(), spec.spec)
        };

        let res = match self.common.output {
            InfoOutput::XdrBase64 => base64,
            InfoOutput::Json => serde_json::to_string(&spec)?,
            InfoOutput::JsonFormatted => serde_json::to_string_pretty(&spec)?,
            InfoOutput::Pretty => contract::pretty_spec(&spec),
        };

        Ok(res)
    }
}
