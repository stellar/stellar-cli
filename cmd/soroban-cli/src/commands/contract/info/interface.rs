use std::fmt::Debug;

use crate::commands::contract::info::interface::Error::NoInterfacePresent;
use crate::commands::contract::info::shared::{self, fetch, Fetched};
use crate::commands::global;
use crate::print::Print;
use crate::xdr::{Limits, WriteXdr};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use clap::Parser;
use soroban_spec_rust::ToFormattedString;
use soroban_spec_tools::contract;
use soroban_spec_tools::contract::Spec;

#[derive(Parser, Debug, Clone)]
pub struct Cmd {
    #[command(flatten)]
    pub common: shared::Args,
    /// Format of the output
    #[arg(long, default_value = "rust")]
    pub output: InfoOutput,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, clap::ValueEnum, Default)]
pub enum InfoOutput {
    /// Rust code output of the contract interface
    #[default]
    Rust,
    /// XDR output of the info entry
    XdrBase64,
    /// JSON output of the info entry (one line, not formatted)
    Json,
    /// Formatted (multiline) JSON output of the info entry
    JsonFormatted,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Wasm(#[from] shared::Error),
    #[error(transparent)]
    Spec(#[from] contract::Error),
    #[error(transparent)]
    ReadSpec(#[from] soroban_spec::read::FromWasmError),
    #[error(transparent)]
    Xdr(#[from] crate::xdr::Error),
    #[error("no interface present in provided WASM file")]
    NoInterfacePresent(),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Generate(#[from] soroban_spec_rust::GenerateError),
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let print = Print::new(global_args.quiet);
        let Fetched { contract, .. } = fetch(&self.common, &print).await?;

        let (base64, spec, sdk_major_version) = match contract {
            shared::Contract::Wasm { wasm_bytes } => {
                let spec = Spec::new(&wasm_bytes)?;

                if spec.env_meta_base64.is_none() {
                    return Err(NoInterfacePresent());
                }

                // Callable functions the contract's spec does not describe are
                // added with `Val` typed inputs and output.
                let mut entries = spec.spec;
                entries.extend(soroban_spec::read::missing_fns_from_wasm(&wasm_bytes)?);
                let mut raw = Vec::new();
                for entry in &entries {
                    raw.extend(entry.to_xdr(Limits::none())?);
                }

                let sdk_major_version = soroban_spec_rust::sdk_major_version_from_meta(&spec.meta);
                (STANDARD.encode(raw), entries, sdk_major_version)
            }
            shared::Contract::StellarAssetContract => {
                let (base64, spec) = Spec::spec_to_base64(stellar_asset_spec::xdr())?;
                (base64, spec, None)
            }
        };

        let res = match self.output {
            InfoOutput::XdrBase64 => base64,
            InfoOutput::Json => serde_json::to_string(&spec)?,
            InfoOutput::JsonFormatted => serde_json::to_string_pretty(&spec)?,
            // soroban_spec_rust drops doc strings entirely (rustdocs can execute
            // code) and routes every spec name through `format_ident!`, which
            // rejects non-identifier bytes. If a future revision starts
            // emitting spec strings as `Literal::string` or rustdocs, this
            // path becomes a terminal-escape-injection vector and must be
            // sanitized before printing.
            InfoOutput::Rust => soroban_spec_rust::generate_without_file(&spec, sdk_major_version)?
                .to_formatted_string()
                .expect("Unexpected spec format error"),
        };

        println!("{res}");

        Ok(())
    }
}
