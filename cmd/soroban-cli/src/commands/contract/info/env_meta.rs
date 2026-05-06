use std::fmt::Debug;
use std::fmt::Write;

use clap::Parser;

use soroban_spec_tools::contract;
use soroban_spec_tools::contract::Spec;

use crate::{
    commands::{
        contract::info::{
            env_meta::Error::{NoEnvMetaPresent, NoSACEnvMeta},
            shared::{self, fetch, Fetched, MetasInfoOutput},
        },
        global,
    },
    print::Print,
    xdr::{ScEnvMetaEntry, ScEnvMetaEntryInterfaceVersion},
};

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
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let print = Print::new(global_args.quiet);
        let Fetched { contract, .. } = fetch(&self.common, &print).await?;

        let spec = match contract {
            shared::Contract::Wasm { wasm_bytes } => Spec::new(&wasm_bytes)?,
            shared::Contract::StellarAssetContract => return Err(NoSACEnvMeta()),
        };

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
                            let _ = writeln!(meta_str, " • Protocol: v{protocol}");
                            if pre_release != &0 {
                                let _ = writeln!(meta_str, " • Pre-release: v{pre_release}");
                            }
                        }
                    }
                }
                meta_str
            }
        };

        println!("{res}");

        Ok(())
    }
}
