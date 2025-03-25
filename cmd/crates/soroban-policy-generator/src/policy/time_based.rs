use crate::error::Error;
use serde_json::Value;

pub fn generate_time_based_policy(params: &Value) -> Result<String, Error> {
    let expiration = params["expiration"]
        .as_u64()
        .ok_or_else(|| Error::InvalidParams("expiration parameter must be a positive integer".into()))?;

    let template = r#"
#![no_std]
use soroban_sdk::{contract, contractimpl, Address, Env};

#[contract]
pub struct TimePolicy;

#[contractimpl]
impl TimePolicy {
    pub fn check_policy(env: Env, source: Address) -> bool {
        env.ledger().timestamp() <= {{expiration}}
    }
}
"#;

    Ok(template.replace("{{expiration}}", &expiration.to_string()))
}