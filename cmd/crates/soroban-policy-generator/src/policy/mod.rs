pub mod time_based;
pub mod amount_based;
pub mod multi_sig;

use serde_json::Value;
use stellar_xdr::curr::ScSpecEntry;
use crate::GenerateError;

/// Common trait for all policy generators
pub trait PolicyGenerator {
    /// Generate policy code from contract spec and parameters
    fn generate(spec: &[ScSpecEntry], params: &Value) -> Result<String, GenerateError>;
}

/// Common functionality for policy generators
pub(crate) mod utils {
    use super::*;
    use crate::types::Entry;
    use handlebars::Handlebars;

    /// Extract contract methods from spec
    pub fn extract_methods(spec: &[ScSpecEntry]) -> Vec<Entry> {
        spec.iter()
            .filter_map(|entry| match Entry::from(entry) {
                Entry::Function { .. } = f => Some(f),
                _ => None,
            })
            .collect()
    }

    /// Render template with given data
    pub fn render_template(
        template_name: &str,
        data: &serde_json::Value,
    ) -> Result<String, GenerateError> {
        let mut reg = Handlebars::new();
        reg.register_template_string(template_name, include_str!("../../templates/policy.rs.hbs"))
            .map_err(GenerateError::Template)?;
        reg.render(template_name, data).map_err(GenerateError::Template)
    }
} 