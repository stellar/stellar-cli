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
        "makefile",
        r#"SOROBAN_RPC_URL ?= https://soroban-testnet.stellar.org
SOROBAN_NETWORK_PASSPHRASE ?= "Test SDF Network ; September 2015"
SOROBAN_SECRET_KEY ?= $(shell cat .soroban/secret_key)
POLICY_ID ?= $(shell cat .soroban/policy_id)

all: build

build:
	@cargo build --target wasm32-unknown-unknown --release
	@mkdir -p .soroban
	@ls -l target/wasm32-unknown-unknown/release/{{policy_name}}.wasm

optimize: build
	@soroban contract optimize \
		--wasm target/wasm32-unknown-unknown/release/{{policy_name}}.wasm \
		--wasm-out .soroban/{{policy_name}}_optimized.wasm

deploy: optimize
	@soroban contract deploy \
		--wasm .soroban/{{policy_name}}_optimized.wasm \
		--source $(SOROBAN_SECRET_KEY) \
		--rpc-url $(SOROBAN_RPC_URL) \
		--network-passphrase $(SOROBAN_NETWORK_PASSPHRASE) \
		> .soroban/policy_id

clean:
	@cargo clean
	@rm -rf .soroban/{{policy_name}}_optimized.wasm

.PHONY: all build optimize deploy clean"#,
    )?;

    handlebars.register_template_string(
        "lib_rs",
        r#"#![no_std]

use smart_wallet_interface::{types::SignerKey, PolicyInterface};
use soroban_sdk::{
    auth::{Context, ContractContext},
    contract, contracterror, contractimpl, panic_with_error, Symbol,
    Address, Env, Vec,
};

#[contracterror]
#[derive(Copy, Clone, Debug, PartialEq)]
#[repr(u32)]
pub enum Error {
    NotAllowed = 1,
}

#[contract]
pub struct Contract;

#[contractimpl]
impl PolicyInterface for Contract {
    fn policy__(env: Env, _source: Address, _signer: SignerKey, contexts: Vec<Context>) {
        for context in contexts.iter() {
            match context {
                Context::Contract(ContractContext { fn_name, .. }) => {
{{#each allowed_methods}}                    if fn_name == Symbol::new(&env, "{{this}}") { return; }
{{/each}}                }
                _ => panic_with_error!(&env, Error::NotAllowed),
            }
        }
        panic_with_error!(&env, Error::NotAllowed)
    }
}"#,
    )?;

    handlebars.register_template_string(
        "function_based_policy",
        r#"{{#each allowed_methods}}                    if fn_name == Symbol::new(&env, "{{this}}") { return; }
{{/each}}"#,
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