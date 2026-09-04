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
            shared::Contract::Wasm { wasm_bytes } => {
                let spec = Spec::new(&wasm_bytes)?;

                if spec.env_meta_base64.is_none() {
                    return Err(NoInterfacePresent());
                }

                (spec.spec_base64.unwrap(), spec.spec)
            }
            shared::Contract::StellarAssetContract => {
                Spec::spec_to_base64(stellar_asset_spec::xdr())?
            }
        };

        // Contract specs may name user-defined types by their fully qualified
        // path (e.g. `my_contract::inner::State`). Those names are noisy and,
        // because `::` is not a valid identifier, the Rust and JSON renderers
        // below cannot use them as-is. Reduce them to short names for display,
        // reporting what changed. The `XdrBase64` output is the canonical
        // on-chain spec, so it is left untouched.
        let reduced = soroban_spec::reduce::reduce(&spec);
        if !matches!(self.output, InfoOutput::XdrBase64) {
            for rename in reduced.renames().filter(|r| r.renamed()) {
                print.infoln(format!(
                    "Reduced type name {} to {}",
                    String::from_utf8_lossy(&rename.from),
                    String::from_utf8_lossy(&rename.to),
                ));
            }
            let collisions: Vec<_> = reduced.renames().filter(|r| r.collision()).collect();
            if !collisions.is_empty() {
                use std::fmt::Write as _;
                let mut msg = String::from(
                    "Reduced type names collided and were disambiguated with a numeric suffix:",
                );
                for rename in collisions {
                    let _ = write!(
                        msg,
                        "\n    {} -> {}",
                        String::from_utf8_lossy(&rename.from),
                        String::from_utf8_lossy(&rename.to),
                    );
                }
                print.warnln(msg);
            }
        }
        let reduced_spec: Vec<_> = reduced.into_entries().collect();

        let res = match self.output {
            InfoOutput::XdrBase64 => base64,
            InfoOutput::Json => serde_json::to_string(&reduced_spec)?,
            InfoOutput::JsonFormatted => serde_json::to_string_pretty(&reduced_spec)?,
            // soroban_spec_rust drops doc strings entirely (rustdocs can execute
            // code) and routes every spec name through `format_ident!`, which
            // rejects non-identifier bytes. If a future revision starts
            // emitting spec strings as `Literal::string` or rustdocs, this
            // path becomes a terminal-escape-injection vector and must be
            // sanitized before printing.
            InfoOutput::Rust => soroban_spec_rust::generate_without_file(&reduced_spec)?
                .to_formatted_string()
                .expect("Unexpected spec format error"),
        };

        println!("{res}");

        Ok(())
    }
}
