use handlebars::Handlebars;
use serde_json::Value;

pub fn register_templates(handlebars: &mut Handlebars) -> Result<(), handlebars::TemplateError> {
    // Disable HTML escaping
    handlebars.register_escape_fn(handlebars::no_escape);

    handlebars.register_template_string(
        "workspace_cargo_toml",
        r#"[workspace]
resolver = "2"

members = ["{{policy_name}}"]

[workspace.dependencies]
soroban-sdk = "22.0.4"
smart-wallet-interface = { git = "https://github.com/kalepail/passkey-kit", branch = "next" }

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
use soroban_sdk::{
    auth::{Context, ContractContext},
    contract, contracterror, contractimpl, panic_with_error, symbol_short,
    Address, Env, Vec,
};
use smart_wallet_interface::{types::SignerKey, PolicyInterface};

#[contracterror]
#[derive(Copy, Clone, Debug, PartialEq)]
#[repr(u32)]
pub enum Error {
    NotAllowed = 1,
}

#[contract]
pub struct {{policy_name}}Contract;

#[contractimpl]
impl PolicyInterface for {{policy_name}}Contract {
    fn policy__(env: Env, _source: Address, _signer: SignerKey, contexts: Vec<Context>) {
{{policy_impl}}
    }
}"#,
    )?;

    handlebars.register_template_string(
        "function_based_policy",
        r#"        for context in contexts.iter() {
            if let Context::Contract(ContractContext { fn_name, .. }) = context {
{{#each allowed_methods}}                if fn_name == symbol_short!("{{this}}") { return; }
{{/each}}            }
        }
        panic_with_error!(&env, Error::NotAllowed)"#,
    )?;

    Ok(())
}

pub fn render_template(handlebars: &Handlebars, template_name: &str, data: &Value) -> Result<String, handlebars::RenderError> {
    handlebars.render(template_name, data)
} 