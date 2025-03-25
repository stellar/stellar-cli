use crate::error::Error;
use handlebars::Handlebars;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize)]
pub struct SmartWalletPolicy {
    pub function_rules: Option<HashMap<String, FunctionRule>>,
    pub context_validation: Option<ContextValidation>,
    pub signer_rules: Option<SignerRules>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FunctionRule {
    pub enabled: bool,
    pub amount_limit: Option<i128>,
    pub require_signer: Option<bool>,
    pub allowed_signers: Option<Vec<String>>,
    pub min_signers: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ContextValidation {
    pub validate_contract_context: bool,
    pub allowed_contracts: Option<Vec<String>>,
    pub contract_rules: Option<HashMap<String, ContractRule>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ContractRule {
    pub max_daily_volume: Option<i128>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SignerRules {
    pub require_specific_signers: bool,
    pub allowed_signers: Option<Vec<String>>,
}

pub struct SmartWalletPolicyGenerator {
    handlebars: Handlebars<'static>,
}

impl SmartWalletPolicyGenerator {
    pub fn new() -> Result<Self, Error> {
        let mut handlebars = Handlebars::new();
        match handlebars.register_template_string(
            "smart_wallet_policy",
            include_str!("../../templates/smart-wallet/policy.rs.hbs"),
        ) {
            Ok(_) => Ok(Self { handlebars }),
            Err(e) => Err(Error::Template(e.to_string())),
        }
    }

    pub fn generate(&self, policy: SmartWalletPolicy) -> Result<String, Error> {
        let template_data = serde_json::to_value(policy)?;
        let generated = self.handlebars.render("smart_wallet_policy", &template_data)?;
        Ok(generated)
    }

    pub fn validate_policy(&self, policy: &SmartWalletPolicy) -> Result<(), Error> {
        // Basic validation
        if let Some(rules) = &policy.function_rules {
            for (_, rule) in rules {
                if rule.enabled {
                    if rule.require_signer.unwrap_or(false) && rule.allowed_signers.is_none() {
                        return Err(Error::ValidationError(
                            "Signer required but no allowed signers specified".to_string(),
                        ));
                    }
                }
            }
        }

        // Context validation
        if let Some(context) = &policy.context_validation {
            if context.validate_contract_context && context.allowed_contracts.is_none() {
                return Err(Error::ValidationError(
                    "Contract context validation enabled but no allowed contracts specified"
                        .to_string(),
                ));
            }
        }

        Ok(())
    }
} 