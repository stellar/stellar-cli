use clap::Parser;
use dialoguer::{Input, Select, Confirm, MultiSelect};
use soroban_policy_generator::{PolicyType, generate_policy};
use soroban_spec_tools::contract::Spec;
use stellar_xdr::curr::ScSpecEntry;
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use crate::commands::contract::info::shared as contract_spec;
use crate::print::Print;
use crate::commands::NetworkRunnable;
use async_trait::async_trait;
use crate::commands::global;
use crate::config;
use crate::config::network;
use crate::config::locator;
use handlebars::Handlebars;

#[derive(Parser, Debug, Clone)]
#[command(name = "generate")]
pub struct Args {
    /// Path to the contract WASM file
    #[arg(long, group = "Source", conflicts_with = "contract_id")]
    wasm: Option<String>,

    /// Contract ID/alias on a network
    #[arg(
        long,
        env = "STELLAR_CONTRACT_ID",
        group = "Source",
        visible_alias = "id",
        conflicts_with = "wasm"
    )]
    contract_id: Option<config::UnresolvedContract>,

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

    #[command(flatten)]
    pub network: network::Args,

    #[command(flatten)]
    pub locator: locator::Args,
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
    #[error("Contract spec error: {0}")]
    ContractSpec(#[from] soroban_spec_tools::contract::Error),
    #[error("Contract fetch error: {0}")]
    ContractFetch(String),
}

impl From<contract_spec::Error> for Error {
    fn from(e: contract_spec::Error) -> Self {
        Error::ContractFetch(e.to_string())
    }
}

impl From<handlebars::TemplateError> for Error {
    fn from(e: handlebars::TemplateError) -> Self {
        Error::PolicyGeneration(e.to_string())
    }
}

#[derive(Parser, Debug, Clone)]
#[command(name = "policy")]
pub enum Cmd {
    /// Generate a policy for a Soroban contract
    Generate(Args),
}

impl Cmd {
    fn create_project_structure(&self, output_dir: &PathBuf, policy_name: &str, policy: &str) -> Result<(), Error> {
        // Create the main output directory
        fs::create_dir_all(output_dir)?;

        // Create the policy project directory
        let policy_dir = output_dir.join(policy_name);
        fs::create_dir_all(&policy_dir)?;

        // Initialize handlebars
        let mut handlebars = Handlebars::new();
        soroban_policy_generator::templates::register_templates(&mut handlebars)?;

        // Create workspace Cargo.toml
        let workspace_data = json!({
            "policy_name": policy_name,
        });
        let workspace_cargo = handlebars.render("workspace_cargo_toml", &workspace_data)
            .map_err(|e| Error::PolicyGeneration(format!("Failed to render workspace Cargo.toml: {}", e)))?;
        fs::write(output_dir.join("Cargo.toml"), workspace_cargo)?;

        // Create policy Cargo.toml
        let policy_cargo = handlebars.render("policy_cargo_toml", &workspace_data)
            .map_err(|e| Error::PolicyGeneration(format!("Failed to render policy Cargo.toml: {}", e)))?;
        fs::write(policy_dir.join("Cargo.toml"), policy_cargo)?;

        // Create src directory
        let src_dir = policy_dir.join("src");
        fs::create_dir_all(&src_dir)?;

        // Create lib.rs with policy implementation
        let lib_data = json!({
            "policy_name": policy_name,
            "policy_impl": policy,
        });
        let lib_rs = handlebars.render("lib_rs", &lib_data)
            .map_err(|e| Error::PolicyGeneration(format!("Failed to render lib.rs: {}", e)))?;
        fs::write(src_dir.join("lib.rs"), lib_rs)?;

        // Create Makefile
        let makefile = handlebars.render("makefile", &workspace_data)
            .map_err(|e| Error::PolicyGeneration(format!("Failed to render Makefile: {}", e)))?;
        fs::write(output_dir.join("Makefile"), makefile)?;

        // Create .soroban directory
        fs::create_dir_all(output_dir.join(".soroban"))?;

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

    async fn extract_contract_functions(&self, args: &Args) -> Result<Vec<String>, Error> {
        let print = Print::new(false);
        
        // Create args for contract spec fetching
        let spec_args = contract_spec::Args {
            wasm: args.wasm.as_ref().map(PathBuf::from),
            wasm_hash: None,
            contract_id: args.contract_id.clone(),
            network: args.network.clone(),
            locator: args.locator.clone(),
        };

        // Fetch the contract spec
        let contract_spec::Fetched { contract, .. } = contract_spec::fetch(&spec_args, &print).await?;

        let functions = match contract {
            contract_spec::Contract::Wasm { wasm_bytes } => {
                let spec = Spec::new(&wasm_bytes)?.spec;
                spec.iter()
                    .filter_map(|entry| {
                        if let ScSpecEntry::FunctionV0(func) = entry {
                            Some(func.name.to_string())
                        } else {
                            None
                        }
                    })
                    .collect()
            },
            contract_spec::Contract::StellarAssetContract => {
                // Known SAC functions - including only state-modifying and admin functions
                vec![
                    // State-modifying functions
                    "transfer".to_string(),
                    "burn".to_string(),
                    "mint".to_string(),
                    "transfer_from".to_string(),

                    // Admin/Authorization functions
                    "set_admin".to_string(),
                    "approve".to_string(),
                    "upgrade".to_string(),
                    "init".to_string(),

                    // Read-only functions (commented out as they don't need policy control)
                    // "balance".to_string(),
                    // "allowance".to_string(),
                    // "decimals".to_string(),
                    // "name".to_string(),
                    // "symbol".to_string(),
                ]
            }
        };
        
        Ok(functions)
    }

    async fn run_interactive_mode(&self, args: &Args) -> Result<(PolicyType, serde_json::Value), Error> {
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
                // Extract available functions from contract
                let functions = self.extract_contract_functions(args).await?;
                
                // Let user select functions to include in policy
                let selections = MultiSelect::new()
                    .with_prompt("Select functions to include in policy")
                    .items(&functions)
                    .interact()
                    .map_err(|e| Error::Interactive(e.to_string()))?;

                let selected_functions: Vec<String> = selections.iter()
                    .map(|&i| functions[i].clone())
                    .collect();

                // For each selected function, configure its rules
                let mut function_rules = serde_json::Map::new();
                for func_name in selected_functions {
                    println!("\nConfiguring rules for function: {}", func_name);
                    
                    let enable = Confirm::new()
                        .with_prompt(format!("Enable {} function?", func_name))
                        .default(true)
                        .interact()
                        .map_err(|e| Error::Interactive(e.to_string()))?;

                    let amount_limit = if enable {
                        let input: String = Input::new()
                            .with_prompt(format!("Enter amount limit for {} (in stroops)", func_name))
                            .default("1000000".into())
                            .interact()
                            .map_err(|e| Error::Interactive(e.to_string()))?;
                        input.parse::<i128>().map_err(|e| Error::Interactive(e.to_string()))?
                    } else {
                        0
                    };

                    let require_signer = Confirm::new()
                        .with_prompt(format!("Require specific signer for {}?", func_name))
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

                    function_rules.insert(func_name, json!({
                        "enabled": enable,
                        "amount_limit": amount_limit,
                        "require_signer": require_signer,
                        "allowed_signers": allowed_signers
                    }));
                }

                json!({
                    "function_rules": function_rules
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
                // Extract available functions from contract
                let functions = self.extract_contract_functions(args).await?;
                
                // Let user select functions to include in policy
                let selections = MultiSelect::new()
                    .with_prompt("Select functions to include in policy")
                    .items(&functions)
                    .interact()
                    .map_err(|e| Error::Interactive(e.to_string()))?;

                let selected_functions: Vec<String> = selections.iter()
                    .map(|&i| functions[i].clone())
                    .collect();

                json!({
                    "method_configs": selected_functions
                })
            }
        };

        Ok((policy_type, params))
    }

    pub async fn run(&self) -> Result<(), Error> {
        self.run_against_rpc_server(None, None).await
    }
}

#[async_trait::async_trait]
impl NetworkRunnable for Cmd {
    type Error = Error;
    type Result = ();

    async fn run_against_rpc_server(
        &self,
        _global_args: Option<&global::Args>,
        _config: Option<&config::Args>,
    ) -> Result<(), Error> {
        match self {
            Cmd::Generate(args) => {
                let (policy_type, params) = if args.interactive.unwrap_or(false) {
                    self.run_interactive_mode(args).await?
                } else {
                    let policy_type = args.policy_type
                        .as_deref()
                        .and_then(|pt| match pt {
                            "time-based" => Some(PolicyType::TimeBased),
                            "function-based" => Some(PolicyType::FunctionBased),
                            "smart-wallet" => Some(PolicyType::SmartWallet),
                            _ => None,
                        })
                        .ok_or_else(|| Error::InvalidPolicyType("Invalid policy type".to_string()))?;

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