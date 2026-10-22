use std::fmt::Debug;

use crate::commands::contract::info::interface::Error::NoInterfacePresent;
use crate::commands::contract::info::shared::{self, fetch, Fetched};
use crate::commands::global;
use crate::print::Print;
use clap::Parser;
use soroban_spec_rust::ToFormattedString;
use soroban_spec_tools::contract;
use soroban_spec_tools::contract::Spec;
use soroban_spec_tools::sanitize;

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
    #[error("{0}")]
    Generate(String),
}

impl Cmd {
    pub fn extract_interface(
        contract: &shared::Contract,
    ) -> Result<(String, Vec<stellar_xdr::ScSpecEntry>, Option<u32>), Error> {
        match contract {
            shared::Contract::Wasm { wasm_bytes } => {
                let spec = Spec::new(wasm_bytes)?;

                let Some(base64) = spec.spec_base64 else {
                    return Err(NoInterfacePresent());
                };

                if spec.env_meta_base64.is_none() {
                    return Err(NoInterfacePresent());
                }

                let sdk_major_version = soroban_spec_rust::sdk_major_version_from_meta(&spec.meta);
                Ok((base64, spec.spec, sdk_major_version))
            }
            shared::Contract::StellarAssetContract => {
                let (base64, spec) = Spec::spec_to_base64(&stellar_asset_spec::xdr())?;
                Ok((base64, spec, None))
            }
        }
    }

    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let print = Print::new(global_args.quiet);
        let Fetched { contract, .. } = fetch(&self.common, &print).await?;

        let (base64, spec, sdk_major_version) = Self::extract_interface(&contract)?;

        let res = match self.output {
            InfoOutput::XdrBase64 => base64,
            InfoOutput::Json => serde_json::to_string(&spec)?,
            InfoOutput::JsonFormatted => serde_json::to_string_pretty(&spec)?,
            // soroban_spec_rust drops doc strings entirely (rustdocs can execute
            // code) and routes every spec name through `format_ident!`, which
            // rejects non-identifier bytes. If a future revision starts
            // emitting spec strings as `Literal::string` or rustdocs, this
            // path becomes a terminal-escape-injection vector and must be
            // sanitized before printing. Names are reduced before they reach
            // `format_ident!`, and reduce errors embed the raw names, so the
            // error is sanitized before it is printed, as the spec is
            // untrusted input.
            InfoOutput::Rust => soroban_spec_rust::generate_without_file(&spec, sdk_major_version)
                .map_err(|e| Error::Generate(sanitize(&e.to_string())))?
                .to_formatted_string()
                .expect("Unexpected spec format error"),
        };

        println!("{res}");

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use stellar_xdr::{
        Limits, ScEnvMetaEntry, ScEnvMetaEntryInterfaceVersion, ScSpecEntry, ScSpecFunctionV0,
        StringM, WriteXdr,
    };

    fn make_custom_section(name: &str, data: &[u8]) -> Vec<u8> {
        let mut payload = Vec::new();
        payload.push(name.len() as u8);
        payload.extend_from_slice(name.as_bytes());
        payload.extend_from_slice(data);

        let mut section = Vec::new();
        section.push(0x00); // Custom section ID
        section.push(payload.len() as u8);
        section.extend(payload);
        section
    }

    fn make_env_meta_bytes() -> Vec<u8> {
        let entry = ScEnvMetaEntry::ScEnvMetaKindInterfaceVersion(ScEnvMetaEntryInterfaceVersion {
            protocol: 21,
            pre_release: 0,
        });
        entry.to_xdr(Limits::none()).unwrap()
    }

    fn make_spec_bytes() -> Vec<u8> {
        let spec_entry = ScSpecEntry::FunctionV0(ScSpecFunctionV0 {
            doc: StringM::default(),
            name: "hello".try_into().unwrap(),
            inputs: Vec::new().try_into().unwrap(),
            outputs: Vec::new().try_into().unwrap(),
        });
        spec_entry.to_xdr(Limits::none()).unwrap()
    }

    #[test]
    fn test_extract_interface_wasm_without_spec_returns_no_interface_error() {
        // A wasm containing contractenvmetav0 but omitting contractspecv0
        // must return Err(NoInterfacePresent) rather than panic on unwrap().
        let mut wasm_bytes = b"\0asm\x01\x00\x00\x00".to_vec();
        wasm_bytes.extend(make_custom_section(
            "contractenvmetav0",
            &make_env_meta_bytes(),
        ));

        let contract = shared::Contract::Wasm { wasm_bytes };
        let result = Cmd::extract_interface(&contract);

        match result {
            Err(Error::NoInterfacePresent()) => {}
            other => panic!("Expected Err(NoInterfacePresent), got {other:?}"),
        }
    }

    #[test]
    fn test_extract_interface_empty_wasm_returns_no_interface_error() {
        let wasm_bytes = b"\0asm\x01\x00\x00\x00".to_vec();
        let contract = shared::Contract::Wasm { wasm_bytes };
        let result = Cmd::extract_interface(&contract);

        match result {
            Err(Error::NoInterfacePresent()) => {}
            other => panic!("Expected Err(NoInterfacePresent), got {other:?}"),
        }
    }

    #[test]
    fn test_extract_interface_stellar_asset_contract_succeeds() {
        let contract = shared::Contract::StellarAssetContract;
        let (base64, spec, sdk_version): (String, Vec<stellar_xdr::ScSpecEntry>, Option<u32>) =
            Cmd::extract_interface(&contract).unwrap();
        assert!(!base64.is_empty());
        assert!(!spec.is_empty());
        assert_eq!(sdk_version, None);
    }

    #[test]
    fn test_extract_interface_wasm_with_spec_and_env_meta_succeeds() {
        let mut wasm_bytes = b"\0asm\x01\x00\x00\x00".to_vec();
        wasm_bytes.extend(make_custom_section(
            "contractenvmetav0",
            &make_env_meta_bytes(),
        ));
        wasm_bytes.extend(make_custom_section("contractspecv0", &make_spec_bytes()));

        let contract = shared::Contract::Wasm { wasm_bytes };
        let (base64, spec, _sdk_version): (String, Vec<stellar_xdr::ScSpecEntry>, Option<u32>) =
            Cmd::extract_interface(&contract).unwrap();
        assert!(!base64.is_empty());
        assert_eq!(spec.len(), 1);
    }
}
