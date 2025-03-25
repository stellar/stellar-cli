use std::path::PathBuf;
use clap::{arg, command, Parser};
use crate::{
    config,
    commands::{global, NetworkRunnable},
    print::Print,
};
use crate::commands::txn_result::TxnResult;
use serde_json::Value;
use async_trait::async_trait;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Config(#[from] config::Error),
    #[error(transparent)]
    Network(#[from] config::network::Error),
    #[error(transparent)]
    Rpc(#[from] soroban_rpc::Error),
    #[error(transparent)]
    Contract(#[from] soroban_spec_tools::contract::Error),
    #[error("unsupported policy type: {0}")]
    UnsupportedPolicyType(String),
    #[error("failed to create directory: {0}")]
    CreateDirError(std::io::Error),
    #[error("failed to write file: {0}")]
    WriteError(std::io::Error),
    #[error("failed to parse JSON parameters: {0}")]
    JsonParseError(#[from] serde_json::Error),
}

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    /// Contract ID to invoke
    #[arg(long = "id", env = "STELLAR_CONTRACT_ID")]
    pub contract_id: config::UnresolvedContract,

    /// Type of policy to generate (time-based, amount-based, multi-sig)
    #[arg(long = "policy-type")]
    pub policy_type: String,

    /// Output directory for the generated policy contract
    #[arg(long = "out-dir")]
    pub out_dir: PathBuf,

    /// Parameters for the policy in JSON format
    #[arg(long = "params")]
    pub params: Option<String>,

    #[command(flatten)]
    pub config: config::Args,

    #[command(flatten)]
    pub fee: crate::fee::Args,
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let result = self.run_against_rpc_server(Some(global_args), None).await?;
        match result {
            TxnResult::Res(msg) => {
                println!("{}", msg);
                Ok(())
            }
            TxnResult::Txn(_) => Ok(()),
        }
    }
}

#[async_trait]
impl NetworkRunnable for Cmd {
    type Error = Error;
    type Result = TxnResult<String>;

    async fn run_against_rpc_server(
        &self,
        args: Option<&global::Args>,
        config: Option<&config::Args>,
    ) -> Result<TxnResult<String>, Error> {
        let config = config.unwrap_or(&self.config);
        let print = Print::new(args.map_or(false, |a| a.quiet));
        let network = config.get_network()?;
        let client = network.rpc_client()?;
        let _source_account = config.source_account().await?;

        // Get the account sequence number
        let _account_details = client
            .get_account(&_source_account.clone().to_string())
            .await?;

        // Create output directory if it doesn't exist
        std::fs::create_dir_all(&self.out_dir).map_err(Error::CreateDirError)?;

        // Generate policy contract based on type
        let policy_contract = match self.policy_type.as_str() {
            "time-based" => generate_time_based_policy(&self.params)?,
            "amount-based" => generate_amount_based_policy(&self.params)?,
            "multi-sig" => generate_multi_sig_policy(&self.params)?,
            "function-based" => {
                let params = parse_params(&self.params)?;
                generate_function_based_policy(&params)?
            }
            _ => return Err(Error::UnsupportedPolicyType(self.policy_type.clone())),
        };

        // Write the policy contract to the output directory
        let policy_file = self.out_dir.join("policy_contract.rs");
        std::fs::write(&policy_file, policy_contract).map_err(Error::WriteError)?;

        print.checkln(format!("Generated policy contract in: {}", self.out_dir.display()));
        Ok(TxnResult::Res(format!("Generated policy contract in: {}", self.out_dir.display())))
    }
}

fn parse_params(params: &Option<String>) -> Result<Value, Error> {
    match params {
        Some(p) => Ok(serde_json::from_str(p)?),
        None => Ok(Value::Null),
    }
}

fn generate_time_based_policy(params: &Option<String>) -> Result<String, Error> {
    let params = parse_params(params)?;
    let duration = params
        .get("duration")
        .and_then(Value::as_u64)
        .unwrap_or(86400); // Default to 24 hours

    Ok(format!(
        r#"use soroban_sdk::{{contract, contractimpl, Address, Env}};

#[contract]
pub struct TimeBasedPolicy;

#[contractimpl]
impl TimeBasedPolicy {{
    pub fn check_policy(env: Env, target: Address) -> bool {{
        let created_at = env.storage().instance().get::<_, u64>(&target).unwrap_or(0);
        if created_at == 0 {{
            env.storage().instance().set(&target, &env.ledger().timestamp());
            return true;
        }}
        
        let elapsed = env.ledger().timestamp() - created_at;
        elapsed <= {duration}
    }}
}}
"#
    ))
}

fn generate_amount_based_policy(params: &Option<String>) -> Result<String, Error> {
    let params = parse_params(params)?;
    let limit = params
        .get("limit")
        .and_then(Value::as_u64)
        .unwrap_or(1000); // Default to 1000 units

    Ok(format!(
        r#"use soroban_sdk::{{contract, contractimpl, Address, Env}};

#[contract]
pub struct AmountBasedPolicy;

#[contractimpl]
impl AmountBasedPolicy {{
    pub fn check_policy(env: Env, target: Address, amount: u64) -> bool {{
        let used = env.storage().instance().get::<_, u64>(&target).unwrap_or(0);
        let new_total = used.saturating_add(amount);
        
        if new_total <= {limit} {{
            env.storage().instance().set(&target, &new_total);
            true
        }} else {{
            false
        }}
    }}
}}
"#
    ))
}

fn generate_multi_sig_policy(params: &Option<String>) -> Result<String, Error> {
    let params = parse_params(params)?;
    let required_signatures = params
        .get("required_signatures")
        .and_then(Value::as_u64)
        .unwrap_or(2); // Default to 2 signatures

    Ok(format!(
        r#"use soroban_sdk::{{contract, contractimpl, Address, Env, Vec}};

#[contract]
pub struct MultiSigPolicy;

#[contractimpl]
impl MultiSigPolicy {{
    pub fn check_policy(env: Env, signatures: Vec<Address>) -> bool {{
        signatures.len() >= {required_signatures}
    }}
}}
"#
    ))
}

fn generate_function_based_policy(params: &Value) -> Result<String, Error> {
    let allowed_function = params
        .get("function_name")
        .and_then(Value::as_str)
        .unwrap_or("do_math"); // Default to "do_math" for compatibility

    Ok(format!(
        r#"#![no_std]
use soroban_sdk::{{
    contract, contracterror, contractimpl, Address, Env, Symbol,
}};

#[contracterror]
#[derive(Copy, Clone, Debug, PartialEq)]
#[repr(u32)]
pub enum Error {{
    NotAllowed = 1,
}}

#[contract]
pub struct FunctionPolicy;

#[contractimpl]
impl FunctionPolicy {{
    pub fn check_policy(env: Env, function_name: Symbol) -> bool {{
        function_name == Symbol::new(&env, "{allowed_function}")
    }}

    pub fn get_allowed_function(env: Env) -> Symbol {{
        Symbol::new(&env, "{allowed_function}")
    }}
}}
"#,
        allowed_function = allowed_function
    ))
}