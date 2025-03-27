#![no_std]
use crate::{error::Error, templates};
use handlebars::Handlebars;
use serde_json::{json, Value};

pub fn generate_function_based_policy(params: &Value) -> Result<String, Error> {
    let method_configs: Vec<String> = serde_json::from_value(params["method_configs"].clone())
        .map_err(|_| Error::InvalidParams("Invalid method_configs format".into()))?;

    let mut handlebars = Handlebars::new();
    templates::register_templates(&mut handlebars)
        .map_err(|e| Error::Template(e.to_string()))?;

    // Generate the full contract directly with the allowed methods
    handlebars
        .render(
            "lib_rs",
            &json!({
                "allowed_methods": method_configs,
            }),
        )
        .map_err(|e| Error::Render(e))
} 