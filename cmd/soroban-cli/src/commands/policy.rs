use clap::{Parser, Subcommand};
use dialoguer::{Input, Select, Confirm};
use soroban_policy_generator::{PolicyType, generate_policy};
use serde_json::json;
use std::fs;
use std::path::PathBuf;

#[derive(Parser, Debug)]
pub struct Args {
    /// Path to the contract WASM file
    #[arg(long)]
    wasm: String,

    /// Policy type (time-based, function-based, smart-wallet)
    #[arg(long)]
    policy_type: Option<String>,

    /// Run in interactive mode
    #[arg(long)]
    interactive: Option<bool>,

    /// Policy parameters in JSON format
    #[arg(long)]
    params: Option<String>,

    /// Output directory for the generated policy
    #[arg(long, default_value = "generated_policy")]
    output_dir: PathBuf,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Failed to generate policy: {0}")]
    PolicyGeneration(String),
    #[error("Invalid policy type: {0}")]
    InvalidPolicyType(String),
    #[error("Interactive mode error: {0}")]
    Interactive(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Subcommand)]
pub enum Cmd {
    /// Generate a policy for a Soroban contract
    Generate(Args),
}

impl Cmd {
    fn create_project_structure(&self, output_dir: &PathBuf, policy_name: &str, policy_content: &str) -> Result<(), Error> {
        // Create the main project directory
        fs::create_dir_all(output_dir)?;
        
        // Create src directory
        let src_dir = output_dir.join("src");
        fs::create_dir_all(&src_dir)?;

        // Write the policy code to src/lib.rs
        fs::write(src_dir.join("lib.rs"), policy_content)?;

        // Create Cargo.toml
        let cargo_toml = format!(
            r#"[package]
name = "{}"
version = "0.0.0"
authors = ["Stellar Development Foundation <info@stellar.org>"]
license = "Apache-2.0"
edition = "2021"
publish = false

[lib]
crate-type = ["cdylib", "rlib"]
doctest = false

[features]
testutils = []

[dependencies]
soroban-sdk = {{ workspace = true }}
smart-wallet-interface = {{ workspace = true }}

[dev-dependencies]
soroban-sdk = {{ workspace = true, features = ["testutils"] }}"#,
            policy_name
        );

        fs::write(output_dir.join("Cargo.toml"), cargo_toml)?;

        // Create .gitignore
        let gitignore = r#"target/
Cargo.lock
"#;
        fs::write(output_dir.join(".gitignore"), gitignore)?;

        Ok(())
    }

    fn get_policy_name(&self, policy_type: &PolicyType) -> Result<String, Error> {
        let default_name = match policy_type {
            PolicyType::SmartWallet => "smart-wallet-policy",
            PolicyType::TimeBased => "time-based-policy",
            PolicyType::FunctionBased => "function-based-policy",
        };

        if let Ok(name) = Input::<String>::new()
            .with_prompt("Enter policy name")
            .default(default_name.to_string())
            .interact() {
            Ok(name)
        } else {
            Ok(default_name.to_string())
        }
    }

    fn run_interactive_mode(&self) -> Result<(PolicyType, serde_json::Value), Error> {
        let policy_types = vec!["smart-wallet", "time-based", "function-based"];
        let selection = Select::new()
            .with_prompt("Select policy type")
            .items(&policy_types)
            .default(0)
            .interact()
            .map_err(|e| Error::Interactive(e.to_string()))?;

        let policy_type = match policy_types[selection] {
            "time-based" => PolicyType::TimeBased,
            "function-based" => PolicyType::FunctionBased,
            "smart-wallet" => PolicyType::SmartWallet,
            _ => return Err(Error::InvalidPolicyType("Invalid policy type selected".to_string())),
        };

        let params = match policy_type {
            PolicyType::SmartWallet => {
                // Configure function rules
                let enable_transfer = Confirm::new()
                    .with_prompt("Enable transfer function policy?")
                    .default(true)
                    .interact()
                    .map_err(|e| Error::Interactive(e.to_string()))?;

                let amount_limit = if enable_transfer {
                    let input: String = Input::new()
                        .with_prompt("Enter amount limit (in stroops)")
                        .default("1000000".into())
                        .interact()
                        .map_err(|e| Error::Interactive(e.to_string()))?;
                    input.parse::<i128>().map_err(|e| Error::Interactive(e.to_string()))?
                } else {
                    0
                };

                let require_signer = Confirm::new()
                    .with_prompt("Require specific signer?")
                    .default(false)
                    .interact()
                    .map_err(|e| Error::Interactive(e.to_string()))?;

                let allowed_signers = if require_signer {
                    let input: String = Input::new()
                        .with_prompt("Enter allowed signer public keys (comma-separated)")
                        .interact()
                        .map_err(|e| Error::Interactive(e.to_string()))?;
                    Some(input.split(',').map(|s| s.trim().to_string()).collect::<Vec<_>>())
                } else {
                    None
                };

                json!({
                    "function_rules": {
                        "transfer": {
                            "enabled": enable_transfer,
                            "amount_limit": amount_limit,
                            "require_signer": require_signer,
                            "allowed_signers": allowed_signers
                        }
                    }
                })
            },
            PolicyType::TimeBased => {
                let interval: u64 = Input::new()
                    .with_prompt("Enter time interval (in seconds)")
                    .default(3600)
                    .interact()
                    .map_err(|e| Error::Interactive(e.to_string()))?;

                json!({
                    "expiration": interval
                })
            },
            PolicyType::FunctionBased => {
                let input: String = Input::new()
                    .with_prompt("Enter function names (comma-separated)")
                    .interact()
                    .map_err(|e| Error::Interactive(e.to_string()))?;

                let functions = input.split(',')
                    .map(|s| s.trim().to_string())
                    .collect::<Vec<_>>();

                json!({
                    "method_configs": functions
                })
            }
        };

        Ok((policy_type, params))
    }

    pub fn run(&self) -> Result<(), Error> {
        match self {
            Cmd::Generate(args) => {
                let (policy_type, params) = if args.interactive.unwrap_or(false) {
                    self.run_interactive_mode()?
                } else {
                    let policy_type = args.policy_type
                        .as_deref()
                        .and_then(|pt| match pt {
                            "time-based" => Some(PolicyType::TimeBased),
                            "function-based" => Some(PolicyType::FunctionBased),
                            "smart-wallet" => Some(PolicyType::SmartWallet),
                            _ => None,
                        })
                        .unwrap_or(PolicyType::SmartWallet);

                    let params = args.params
                        .as_deref()
                        .and_then(|p| serde_json::from_str(p).ok())
                        .unwrap_or_else(|| serde_json::json!({}));

                    (policy_type, params)
                };

                match generate_policy(policy_type.clone(), params) {
                    Ok(policy) => {
                        let policy_name = self.get_policy_name(&policy_type)?;
                        let output_dir = &args.output_dir;
                        
                        self.create_project_structure(output_dir, &policy_name, &policy)?;
                        
                        println!("✨ Policy project created successfully at: {}", output_dir.display());
                        println!("To build the policy:");
                        println!("  cd {}", output_dir.display());
                        println!("  cargo build --target wasm32-unknown-unknown --release");
                        Ok(())
                    },
                    Err(e) => Err(Error::PolicyGeneration(e.to_string())),
                }
            }
        }
    }
} 