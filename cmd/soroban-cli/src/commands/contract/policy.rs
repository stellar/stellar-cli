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
use soroban_policy_generator as policy_gen;

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
    #[error("policy generation error: {0}")]
    PolicyGeneration(#[from] policy_gen::Error),
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

        // Generate policy contract
        let policy_contract = policy_gen::generate_policy_contract(
            &soroban_spec::read::from_wasm(&[])?, // Empty WASM for now, as we don't need it
            &self.policy_type,
            self.params.as_deref(),
        )?;

        // Write the policy contract to the output directory
        let policy_file = self.out_dir.join("policy_contract.rs");
        std::fs::write(&policy_file, policy_contract).map_err(Error::WriteError)?;

        print.checkln(format!("Generated policy contract in: {}", self.out_dir.display()));
        Ok(TxnResult::Res(format!("Generated policy contract in: {}", self.out_dir.display())))
    }
}