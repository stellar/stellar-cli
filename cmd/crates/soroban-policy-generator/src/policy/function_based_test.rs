#[cfg(test)]
mod tests {
    use super::function_based::generate_function_based_policy;
    use serde_json::json;

    #[test]
    fn test_function_based_policy_generation() {
        // Test with a single method
        let params = json!({
            "method_configs": ["do_math"]
        });

        let result = generate_function_based_policy(&params).unwrap();
        
        // Verify that the generated policy includes the specified function
        assert!(result.contains("if fn_name == symbol_short!(\"do_math\") { return; }"));
        
        // Test with multiple methods
        let params = json!({
            "method_configs": ["do_math", "transfer", "mint"]
        });

        let result = generate_function_based_policy(&params).unwrap();
        
        // Verify that the generated policy includes all specified functions
        assert!(result.contains("if fn_name == symbol_short!(\"do_math\") { return; }"));
        assert!(result.contains("if fn_name == symbol_short!(\"transfer\") { return; }"));
        assert!(result.contains("if fn_name == symbol_short!(\"mint\") { return; }"));
    }
} 