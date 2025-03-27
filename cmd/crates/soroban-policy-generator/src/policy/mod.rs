mod function_based;
mod smart_wallet;

#[cfg(test)]
mod smart_wallet_test;

#[cfg(test)]
mod function_based_test;

use crate::error::Error;
use serde_json::Value;

#[derive(Debug, Clone)]
pub enum PolicyType {
    FunctionBased,
    SmartWallet,
}

impl PolicyType {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "function-based" => Some(Self::FunctionBased),
            "smart-wallet" => Some(Self::SmartWallet),
            _ => None,
        }
    }
}

pub fn generate_policy(policy_type: PolicyType, params: &Value) -> Result<String, Error> {
    match policy_type {
        PolicyType::FunctionBased => function_based::generate_function_based_policy(params),
        PolicyType::SmartWallet => {
            let policy: smart_wallet::SmartWalletPolicy = serde_json::from_value(params.clone())?;
            let generator = smart_wallet::SmartWalletPolicyGenerator::new()?;
            generator.validate_policy(&policy)?;
            generator.generate(policy)
        }
    }
}