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
    
    match policy_type {
        "time-based" => generate_time_based_policy(&functions, params),
        "amount-based" => generate_amount_based_policy(&functions, params),
        "multi-sig" => generate_multi_sig_policy(&functions, params),
        "function-based" => generate_function_based_policy(&functions, params),
        _ => Err(Error::UnsupportedPolicyType(policy_type.to_string())),
    }
}

pub fn generate_time_based_policy(functions: &[ScSpecFunctionV0], params: Option<&Value>) -> Result<String, Error> {
    let default_duration = params.and_then(|p| p.get("duration"))
        .and_then(|d| d.as_u64())
        .unwrap_or(86400); // Default 24 hours in seconds

    Ok(format!(
        r#"#![no_std]
use soroban_sdk::{{contract, contractimpl, Address, Env}};

#[contract]
pub struct TimeBasedPolicy;

#[contractimpl]
impl TimeBasedPolicy {{
    pub fn check_policy(env: Env, target: Address) -> bool {{
        let now = env.ledger().timestamp();
        let created = env.storage().instance().get(&target).unwrap_or(0);
        
        if created == 0 {{
            env.storage().instance().set(&target, &now);
            return true;
        }}
        
        now >= created + {duration}
    }}

    pub fn get_remaining_time(env: Env, target: Address) -> i64 {{
        let now = env.ledger().timestamp();
        let created = env.storage().instance().get(&target).unwrap_or(now);
        let deadline = created + {duration};
        
        if now >= deadline {{
            0
        }} else {{
            deadline - now
        }}
    }}
}}
"#,
        duration = default_duration
    ))
}

pub fn generate_amount_based_policy(functions: &[ScSpecFunctionV0], params: Option<&Value>) -> Result<String, Error> {
    let default_limit = params.and_then(|p| p.get("limit"))
        .and_then(|l| l.as_u64())
        .unwrap_or(1000); // Default 1000 units

    Ok(format!(
        r#"#![no_std]
use soroban_sdk::{{contract, contractimpl, Address, Env}};

#[contract]
pub struct AmountBasedPolicy;

#[contractimpl]
impl AmountBasedPolicy {{
    pub fn check_policy(env: Env, target: Address, amount: i128) -> bool {{
        let used = env.storage().instance().get(&target).unwrap_or(0_i128);
        let new_total = used + amount;
        
        if new_total <= {limit} {{
            env.storage().instance().set(&target, &new_total);
            true
        }} else {{
            false
        }}
    }}

    pub fn get_remaining_amount(env: Env, target: Address) -> i128 {{
        let used = env.storage().instance().get(&target).unwrap_or(0_i128);
        {limit} - used
    }}
}}
"#,
        limit = default_limit
    ))
}

pub fn generate_multi_sig_policy(functions: &[ScSpecFunctionV0], params: Option<&Value>) -> Result<String, Error> {
    let required_sigs = params.and_then(|p| p.get("required_signatures"))
        .and_then(|s| s.as_u64())
        .unwrap_or(2); // Default 2 signatures required

    Ok(format!(
        r#"#![no_std]
use soroban_sdk::{{contract, contractimpl, Address, Env, Vec}};

#[contract]
pub struct MultiSigPolicy;

#[contractimpl]
impl MultiSigPolicy {{
    pub fn check_policy(env: Env, target: Address, signatures: Vec<Address>) -> bool {{
        signatures.len() >= {required_sigs}
    }}

    pub fn get_required_signatures(_env: Env) -> u32 {{
        {required_sigs}
    }}
}}
"#,
        required_sigs = required_sigs
    ))
}

pub fn generate_function_based_policy(functions: &[ScSpecFunctionV0], params: Option<&Value>) -> Result<String, Error> {
    let allowed_function = params.and_then(|p| p.get("function_name"))
        .and_then(|f| f.as_str())
        .unwrap_or("do_math"); // Default to "do_math" for compatibility

    Ok(format!(
        r#"#![no_std]
use soroban_sdk::{{
    contract, contracterror, contractimpl, Address, Env, Symbol,
}};

#[contracterror]
#[derive(Copy, Clone, Debug, PartialEq)]
#[repr(u32)]
pub enum Error {{
    NotAllowed = 1,
}}

#[contract]
pub struct FunctionPolicy;

#[contractimpl]
impl FunctionPolicy {{
    pub fn check_policy(env: Env, function_name: Symbol) -> bool {{
        function_name == Symbol::new(&env, "{allowed_function}")
    }}

    pub fn get_allowed_function(env: Env) -> Symbol {{
        Symbol::new(&env, "{allowed_function}")
    }}
}}
"#,
        allowed_function = allowed_function
    ))
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