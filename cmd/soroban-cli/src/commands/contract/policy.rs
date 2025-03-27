use async_trait::async_trait;
use clap::Parser;
use soroban_policy_generator as policy_gen;
use std::path::PathBuf;
use crate::{
    commands::{global, NetworkRunnable, txn_result::TxnResult},
    config,
    print::Print,
};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Config(#[from] config::Error),
    #[error(transparent)]
    Network(#[from] config::network::Error),
    #[error(transparent)]
    Rpc(#[from] soroban_rpc::Error),
    #[error(transparent)]
    Contract(#[from] soroban_spec_tools::contract::Error),
    #[error(transparent)]
    PolicyGeneration(#[from] policy_gen::error::Error),
}

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    /// Contract ID to invoke
    #[arg(long = "id")]
    id: String,

    /// Type of policy to generate (smart-wallet, function-based)
    #[arg(long = "policy-type")]
    policy_type: String,

    /// Output directory for the generated policy contract
    #[arg(long = "out-dir")]
    out_dir: PathBuf,

    /// Parameters for the policy in JSON format
    #[arg(long = "params")]
    params: String,

    #[arg(long = "source")]
    source: String,

    #[arg(long = "network")]
    network: String,

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
        std::fs::create_dir_all(&self.out_dir).map_err(|e| policy_gen::error::Error::Io(e))?;

        let params: serde_json::Value = serde_json::from_str(&self.params)
            .map_err(|e| policy_gen::error::Error::InvalidParams(e.to_string()))?;

        let policy_type = policy_gen::PolicyType::from_str(&self.policy_type)
            .ok_or_else(|| policy_gen::error::Error::InvalidParams("Invalid policy type".into()))?;

        let policy = policy_gen::generate_policy(policy_type, params)?;
        policy_gen::write_policy_to_file(&policy, self.out_dir.to_str().unwrap())?;

        print.checkln(format!("Generated policy contract in: {}", self.out_dir.display()));
        Ok(TxnResult::Res(format!("Generated policy contract in: {}", self.out_dir.display())))
    }
}