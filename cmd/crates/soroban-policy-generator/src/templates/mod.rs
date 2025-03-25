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
pub struct Contract;

#[contractimpl]
impl Contract {
    pub fn validate(_env: Env, _source: Address, _signer: SignerKey, _contexts: Vec<Context>) {
{{policy_impl}}
    }
}

#[contractimpl]
impl PolicyInterface for Contract {}"#,
    )?;

    handlebars.register_template_string(
        "function_based_policy",
        r#"        for context in _contexts.iter() {
            if let Context::Contract(ContractContext { fn_name, .. }) = context {
{{#each allowed_methods}}                if fn_name == symbol_short!("{{truncate this 9}}") { return; }
{{/each}}            }
        }
        panic_with_error!(&_env, Error::NotAllowed)"#,
    )?;

    // Register helper for uppercase first letter
    handlebars.register_helper(
        "uppercase_first",
        Box::new(
            |h: &handlebars::Helper,
             _: &handlebars::Handlebars,
             _: &handlebars::Context,
             _: &mut handlebars::RenderContext,
             out: &mut dyn handlebars::Output|
             -> handlebars::HelperResult {
                let param = h.param(0).unwrap().value().as_str().unwrap_or("");
                if let Some(c) = param.chars().next() {
                    out.write(&c.to_uppercase().to_string())?;
                    out.write(&param[c.len_utf8()..])?;
                }
                Ok(())
            },
        ),
    );

    // Register helper for truncating strings
    handlebars.register_helper(
        "truncate",
        Box::new(
            |h: &handlebars::Helper,
             _: &handlebars::Handlebars,
             _: &handlebars::Context,
             _: &mut handlebars::RenderContext,
             out: &mut dyn handlebars::Output|
             -> handlebars::HelperResult {
                let param = h.param(0).unwrap().value().as_str().unwrap_or("");
                let length = h.param(1).unwrap().value().as_u64().unwrap_or(9) as usize;
                out.write(&param.chars().take(length).collect::<String>())?;
                Ok(())
            },
        ),
    );

    Ok(())
}

pub fn render_template(handlebars: &Handlebars, template_name: &str, data: &Value) -> Result<String, handlebars::RenderError> {
    handlebars.render(template_name, data)
} 