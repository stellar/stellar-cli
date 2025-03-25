#[cfg(test)]
mod tests {
    use crate::policy::smart_wallet::{
        ContextValidation, FunctionRule, SmartWalletPolicy, SmartWalletPolicyGenerator,
    };
    use serde_json::json;
    use std::collections::HashMap;

    const TEST_AMOUNT: i128 = 100_000_000_000;

    #[test]
    fn test_basic_policy_generation() {
        let policy = SmartWalletPolicy {
            function_rules: Some(HashMap::from([(
                "transfer".to_string(),
                FunctionRule {
                    enabled: true,
                    amount_limit: Some(TEST_AMOUNT),
                    require_signer: Some(false),
                    allowed_signers: None,
                    min_signers: None,
                },
            )])),
            context_validation: None,
            signer_rules: None,
        };

        let generator = SmartWalletPolicyGenerator::new().unwrap();
        let result = generator.generate(policy);
        assert!(result.is_ok());
    }

    #[test]
    fn test_policy_validation() {
        let policy = SmartWalletPolicy {
            function_rules: Some(HashMap::from([(
                "transfer".to_string(),
                FunctionRule {
                    enabled: true,
                    amount_limit: None,
                    require_signer: Some(true), // Requires signer but no allowed signers
                    allowed_signers: None,
                    min_signers: None,
                },
            )])),
            context_validation: None,
            signer_rules: None,
        };

        let generator = SmartWalletPolicyGenerator::new().unwrap();
        let result = generator.validate_policy(&policy);
        assert!(result.is_err());
    }

    #[test]
    fn test_context_validation() {
        let policy = SmartWalletPolicy {
            function_rules: None,
            context_validation: Some(ContextValidation {
                validate_contract_context: true,
                allowed_contracts: Some(vec!["contract1".to_string(), "contract2".to_string()]),
                contract_rules: None,
            }),
            signer_rules: None,
        };

        let generator = SmartWalletPolicyGenerator::new().unwrap();
        let result = generator.validate_policy(&policy);
        assert!(result.is_ok());
    }

    #[test]
    fn test_full_policy_generation() {
        let policy_json = json!({
            "function_rules": {
                "transfer": {
                    "enabled": true,
                    "amount_limit": TEST_AMOUNT,
                    "require_signer": true,
                    "allowed_signers": ["GDHT...", "GBXG..."]
                }
            },
            "context_validation": {
                "validate_contract_context": true,
                "allowed_contracts": ["contract1", "contract2"]
            },
            "signer_rules": {
                "require_specific_signers": true,
                "allowed_signers": ["GDHT...", "GBXG..."]
            }
        });

        let policy: SmartWalletPolicy = serde_json::from_value(policy_json).unwrap();
        let generator = SmartWalletPolicyGenerator::new().unwrap();
        let result = generator.generate(policy);
        assert!(result.is_ok());
    }
} 