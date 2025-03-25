use serde_json::{json, Value};
use stellar_xdr::curr::ScSpecEntry;

use crate::GenerateError;
use super::{PolicyGenerator, utils};

pub struct TimeBasedPolicyGenerator;

impl PolicyGenerator for TimeBasedPolicyGenerator {
    fn generate(spec: &[ScSpecEntry], params: &Value) -> Result<String, GenerateError> {
        // Extract methods from spec
        let methods = utils::extract_methods(spec);
        
        // Extract parameters
        let interval = params["interval"]
            .as_u64()
            .unwrap_or(86400); // Default: 24 hours
        
        let max_calls = params["max_calls_per_interval"]
            .as_u64()
            .unwrap_or(5); // Default: 5 calls
        
        // Prepare template data
        let data = json!({
            "methods": methods,
            "interval": interval,
            "max_calls": max_calls,
            "method_configs": params["methods"].as_object().unwrap_or(&serde_json::Map::new()),
        });

        // Render template
        utils::render_template("time_based_policy", &data)
    }
}

// Public function for easier access
pub fn generate(spec: &[ScSpecEntry], params: &Value) -> Result<String, GenerateError> {
    TimeBasedPolicyGenerator::generate(spec, params)
} 