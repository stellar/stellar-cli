#![allow(
    clippy::missing_errors_doc,
    clippy::must_use_candidate,
    clippy::missing_panics_doc
)]

use std::{fs, io};
use stellar_xdr::curr::{ScSpecEntry, WriteXdr};

pub mod policy;
pub mod templates;
pub mod types;

#[derive(thiserror::Error, Debug)]
pub enum GenerateError {
    #[error("reading file: {0}")]
    Io(io::Error),
    #[error("parsing contract spec: {0}")]
    Parse(stellar_xdr::curr::Error),
    #[error("getting contract spec: {0}")]
    GetSpec(soroban_spec::read::FromWasmError),
    #[error("template error: {0}")]
    Template(handlebars::RenderError),
    #[error("invalid policy type: {0}")]
    InvalidPolicyType(String),
}

/// Generate a policy contract from a WASM file
pub fn generate_from_file(
    file: &str,
    policy_type: &str,
    params: Option<&str>,
) -> Result<String, GenerateError> {
    // Read file
    let wasm = fs::read(file).map_err(GenerateError::Io)?;

    // Generate code
    generate_from_wasm(&wasm, policy_type, params)
}

/// Generate a policy contract from WASM bytes
pub fn generate_from_wasm(
    wasm: &[u8],
    policy_type: &str,
    params: Option<&str>,
) -> Result<String, GenerateError> {
    // Extract contract spec
    let spec = soroban_spec::read::from_wasm(wasm).map_err(GenerateError::GetSpec)?;
    
    // Generate policy code
    generate(&spec, policy_type, params)
}

/// Generate a policy contract from contract spec
pub fn generate(
    spec: &[ScSpecEntry],
    policy_type: &str,
    params: Option<&str>,
) -> Result<String, GenerateError> {
    // Parse policy parameters
    let params = if let Some(params) = params {
        serde_json::from_str(params).map_err(|e| GenerateError::Parse(e.into()))?
    } else {
        serde_json::Value::Null
    };

    // Generate policy based on type
    match policy_type {
        "time-based" => policy::time_based::generate(spec, &params),
        "amount-based" => policy::amount_based::generate(spec, &params),
        "multi-sig" => policy::multi_sig::generate(spec, &params),
        _ => Err(GenerateError::InvalidPolicyType(policy_type.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_time_based_policy() {
        // TODO: Add tests
    }

    #[test]
    fn test_generate_amount_based_policy() {
        // TODO: Add tests
    }

    #[test]
    fn test_generate_multi_sig_policy() {
        // TODO: Add tests
    }
} 