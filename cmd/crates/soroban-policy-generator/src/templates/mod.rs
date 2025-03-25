use handlebars::Handlebars;
use serde_json::Value;

pub fn register_templates(handlebars: &mut Handlebars) -> Result<(), handlebars::TemplateError> {
    handlebars.register_template_string("policy", include_str!("policy.rs.hbs"))?;
    Ok(())
}

pub fn render_template(handlebars: &Handlebars, template_name: &str, data: &Value) -> Result<String, handlebars::RenderError> {
    handlebars.render(template_name, data)
} 