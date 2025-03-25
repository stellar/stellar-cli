use handlebars::Handlebars;
use serde_json::Value;

pub fn register_templates(handlebars: &mut Handlebars) -> Result<(), handlebars::TemplateError> {
    handlebars.register_template_string(
        "workspace_cargo_toml",
        r#"[workspace]
resolver = "2"

members = ["{{policy_name}}"]

[workspace.dependencies]
soroban-sdk = "22.0.4"
smart-wallet-interface = { git = "https://github.com/stellar/stellar-smart-wallet", branch = "main" }

[profile.release]
opt-level = "z"
overflow-checks = true
debug = 0
strip = "symbols"
debug-assertions = false
panic = "abort"
codegen-units = 1
lto = true"#,
    )?;

    handlebars.register_template_string(
        "policy_cargo_toml",
        r#"[package]
name = "{{policy_name}}"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
soroban-sdk = { workspace = true }
smart-wallet-interface = { workspace = true }

[dev-dependencies]
soroban-sdk = { workspace = true, features = ["testutils"] }

[profile.release]
inherit = true"#,
    )?;

    handlebars.register_template_string(
        "lib_rs",
        r#"#![no_std]
use soroban_sdk::{contract, contractimpl, Env};
use smart_wallet_interface::PolicyTrait;

#[contract]
pub struct {{policy_name}};

#[contractimpl]
impl PolicyTrait for {{policy_name}} {
    {{policy_impl}}
}"#,
    )?;

    Ok(())
}

pub fn render_template(handlebars: &Handlebars, template_name: &str, data: &Value) -> Result<String, handlebars::RenderError> {
    handlebars.render(template_name, data)
} 