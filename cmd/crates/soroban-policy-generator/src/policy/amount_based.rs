use crate::error::Error;
use serde_json::Value;

pub fn generate_amount_based_policy(params: &Value) -> Result<String, Error> {
    let amount = params["amount"]
        .as_u64()
        .ok_or_else(|| Error::InvalidParams("amount parameter must be a positive integer".into()))?;

    let template = r#"
#![no_std]
use soroban_sdk::{contract, contractimpl, Address, Env};

#[contract]
pub struct AmountPolicy;

#[contractimpl]
impl AmountPolicy {
    pub fn check_policy(env: Env, source: Address, amount: i128) -> bool {
        amount <= {{amount}} as i128
    }
}
"#;

    Ok(template.replace("{{amount}}", &amount.to_string()))
} 