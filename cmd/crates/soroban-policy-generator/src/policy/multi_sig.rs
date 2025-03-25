use crate::error::Error;
use serde_json::Value;

pub fn generate_multi_sig_policy(params: &Value) -> Result<String, Error> {
    let required_signatures = params["required_signatures"]
        .as_u64()
        .ok_or_else(|| Error::InvalidParams("required_signatures parameter must be a positive integer".into()))?;

    let template = r#"
#![no_std]
use soroban_sdk::{contract, contractimpl, Address, Env, Vec};

#[contract]
pub struct MultiSigPolicy;

#[contractimpl]
impl MultiSigPolicy {
    pub fn check_policy(env: Env, source: Address, signatures: Vec<Address>) -> bool {
        signatures.len() >= {{required_signatures}}
    }
}
"#;

    Ok(template.replace("{{required_signatures}}", &required_signatures.to_string()))
} 