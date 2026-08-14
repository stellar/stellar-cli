use std::fmt::Debug;

use crate::commands::contract::info::interface::Error::NoInterfacePresent;
use crate::commands::contract::info::shared::{self, fetch, Fetched};
use crate::commands::global;
use crate::print::Print;
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

        let (base64, spec) = match contract {
            shared::Contract::Wasm { wasm_bytes } => spec_from_wasm(&wasm_bytes)?,
            shared::Contract::StellarAssetContract => {
                Spec::spec_to_base64(stellar_asset_spec::xdr())?
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
            InfoOutput::Rust => soroban_spec_rust::generate_without_file(&spec)?
                .to_formatted_string()
                .expect("Unexpected spec format error"),
        };

        println!("{res}");

        Ok(())
    }
}

fn spec_from_wasm(wasm_bytes: &[u8]) -> Result<(String, Vec<crate::xdr::ScSpecEntry>), Error> {
    let spec = Spec::new(wasm_bytes)?;

    if spec.env_meta_base64.is_none() {
        return Err(NoInterfacePresent());
    }

    // `contractenvmetav0` and `contractspecv0` are independent custom
    // sections, so the presence of env meta does not imply a spec is present.
    let Some(base64) = spec.spec_base64 else {
        return Err(NoInterfacePresent());
    };

    Ok((base64, spec.spec))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::borrow::Cow;

    /// A minimal WASM module containing only the given custom sections.
    fn wasm_with_custom_sections(sections: &[(&str, &[u8])]) -> Vec<u8> {
        let mut module = wasm_encoder::Module::new();
        for (name, data) in sections {
            module.section(&wasm_encoder::CustomSection {
                name: Cow::Borrowed(name),
                data: Cow::Borrowed(data),
            });
        }
        module.finish()
    }

    #[test]
    fn env_meta_without_spec_returns_no_interface_error() {
        // Regression test for https://github.com/stellar/stellar-cli/issues/2429:
        // `contractenvmetav0` and `contractspecv0` are independent custom
        // sections, so a WASM can carry env meta without a spec. This used to
        // panic on `spec.spec_base64.unwrap()`.
        let wasm = wasm_with_custom_sections(&[("contractenvmetav0", &[])]);

        let result = spec_from_wasm(&wasm);

        assert!(matches!(result, Err(Error::NoInterfacePresent())));
    }

    #[test]
    fn missing_env_meta_returns_no_interface_error() {
        let wasm = wasm_with_custom_sections(&[]);

        let result = spec_from_wasm(&wasm);

        assert!(matches!(result, Err(Error::NoInterfacePresent())));
    }
}
