use crate::error::Error;
use crate::types::MethodConfig;
use serde_json::Value;

pub fn generate_function_based_policy(params: &Value) -> Result<String, Error> {
    let method_configs: Vec<MethodConfig> = serde_json::from_value(params["method_configs"].clone())
        .map_err(|_| Error::InvalidParams("Invalid method_configs format".into()))?;

    let mut match_arms = String::new();
    for config in method_configs {
        match_arms.push_str(&format!(
            r#""{}" => {},"#,
            config.name, config.allowed
        ));
    }

    let template = format!(
        r#"
#![no_std]
use soroban_sdk::{{contract, contractimpl, Address, Env, Symbol}};

#[contract]
pub struct FunctionPolicy;

#[contractimpl]
impl FunctionPolicy {{
    pub fn check_policy(env: Env, source: Address, function: Symbol) -> bool {{
        match function.to_string().as_str() {{
            {}
            _ => false,
        }}
    }}
}}
"#,
        match_arms
    );

    Ok(template)
} 