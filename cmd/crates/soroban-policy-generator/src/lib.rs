#![allow(
    clippy::missing_errors_doc,
    clippy::must_use_candidate,
    clippy::missing_panics_doc
)]

use std::{fs, io};
use stellar_xdr::curr::{ScSpecEntry, WriteXdr};
use soroban_spec_tools::contract::{Spec, ScSpecFunctionV0};
use serde_json::Value;
use thiserror::Error;
use handlebars::Handlebars;
use serde::Serialize;

pub mod policy;
pub mod templates;
pub mod types;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Failed to generate policy: {0}")]
    PolicyGeneration(String),
    #[error("Invalid parameters: {0}")]
    InvalidParameters(#[from] serde_json::Error),
    #[error("Unsupported policy type: {0}")]
    UnsupportedPolicyType(String),
    #[error(transparent)]
    Spec(#[from] soroban_spec_tools::contract::Error),
    #[error("Template error: {0}")]
    Template(#[from] handlebars::TemplateError),
    #[error("Template render error: {0}")]
    TemplateRender(#[from] handlebars::RenderError),
}

#[derive(Serialize)]
struct MethodConfig {
    enabled: bool,
    max_amount: Option<u64>,
    interval: Option<u64>,
}

#[derive(Serialize)]
struct TemplateData {
    methods: Vec<String>,
    method_configs: std::collections::HashMap<String, MethodConfig>,
    interval: u64,
}

/// Generate a policy contract from a WASM file
pub fn generate_from_file(
    file: &str,
    policy_type: &str,
    params: Option<&str>,
) -> Result<String, Error> {
    // Read file
    let wasm = fs::read(file).map_err(|e| Error::PolicyGeneration(e.to_string()))?;

    // Generate code
    generate_from_wasm(&wasm, policy_type, params)
}

/// Generate a policy contract from WASM bytes
pub fn generate_from_wasm(
    wasm: &[u8],
    policy_type: &str,
    params: Option<&str>,
) -> Result<String, Error> {
    // Extract contract spec
    let spec = soroban_spec::read::from_wasm(wasm).map_err(|e| Error::Spec(e))?;
    
    // Generate policy code
    generate_policy_contract(&spec, policy_type, params)
}

/// Generate a policy contract from contract spec
pub fn generate_policy_contract(
    spec: &Spec,
    policy_type: &str,
    params: Option<&str>,
) -> Result<String, Error> {
    let functions = spec.find_functions()?;
    let method_names: Vec<String> = functions.iter()
        .map(|f| f.name.to_string())
        .collect();

    let params: Value = params
        .map(serde_json::from_str)
        .transpose()?
        .unwrap_or(serde_json::json!({}));

    let interval = params.get("interval")
        .and_then(|i| i.as_u64())
        .unwrap_or(3600); // Default 1 hour

    let mut method_configs = std::collections::HashMap::new();
    if let Some(configs) = params.get("method_configs") {
        if let Some(obj) = configs.as_object() {
            for (name, config) in obj {
                method_configs.insert(name.clone(), MethodConfig {
                    enabled: config.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true),
                    max_amount: config.get("max_amount").and_then(|v| v.as_u64()),
                    interval: config.get("interval").and_then(|v| v.as_u64()),
                });
            }
        }
    }

    let template_data = TemplateData {
        methods: method_names,
        method_configs,
        interval,
    };

    let mut handlebars = Handlebars::new();
    handlebars.register_template_string("policy", include_str!("../templates/policy.rs.hbs"))?;
    
    Ok(handlebars.render("policy", &template_data)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_policy_contract() {
        let spec = Spec {
            functions: vec![
                ScSpecFunctionV0 {
                    name: "do_math".to_string(),
                    ..Default::default()
                },
                ScSpecFunctionV0 {
                    name: "transfer".to_string(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

        let params = r#"{
            "interval": 1800,
            "method_configs": {
                "do_math": {"enabled": true},
                "transfer": {"enabled": true, "max_amount": 1000}
            }
        }"#;

        let result = generate_policy_contract(&spec, "function-based", Some(params));
        assert!(result.is_ok());
    }
} 