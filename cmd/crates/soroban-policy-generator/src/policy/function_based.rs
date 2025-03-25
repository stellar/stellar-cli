use crate::error::Error;
use serde_json::Value;

pub fn generate_function_based_policy(params: &Value) -> Result<String, Error> {
    let method_configs: Vec<String> = serde_json::from_value(params["method_configs"].clone())
        .map_err(|_| Error::InvalidParams("Invalid method_configs format".into()))?;

    let mut match_arms = String::new();
    for method in &method_configs {
        match_arms.push_str(&format!(
            r#"if fn_name == symbol_short!("{}") {{ return; }}"#,
            method
        ));
    }

    let template = format!(
        r#"#![no_std]
use soroban_sdk::{{
    auth::{{Context, ContractContext}},
    contract, contracterror, contractimpl, panic_with_error, symbol_short,
    Address, Env, Vec,
}};
use smart_wallet_interface::{{types::SignerKey, PolicyInterface}};

#[contracterror]
#[derive(Copy, Clone, Debug, PartialEq)]
#[repr(u32)]
pub enum Error {{
    NotAllowed = 1,
}}

#[contract]
pub struct Contract;

#[contractimpl]
impl PolicyInterface for Contract {{
    fn policy__(env: Env, _source: Address, _signer: SignerKey, contexts: Vec<Context>) {{
        for context in contexts.iter() {{
            if let Context::Contract(ContractContext {{ fn_name, .. }}) = context {{
                {}
            }}
        }}
        panic_with_error!(&env, Error::NotAllowed)
    }}
}}"#,
        match_arms
    );

    Ok(template)
} 