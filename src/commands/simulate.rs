use clap::Parser;
use serde::{Deserialize, Serialize};

use crate::commands::{network, CmdClone};
use crate::error::{self, Error};
use crate::session_environment::get_default_session_environments;
use crate::utils::{
    create_and_fund_test_account, get_horizon_url, load_source_wasm_file,
    source_wasm_to_base64,
};
use crate::{
    account::{Account, Network},
    config::{self, network},
    utils::read_file_to_base64,
};

#[derive(Parser, Clone, Debug, Serialize, Deserialize)]
pub struct SimulateCmd {
    /// The path or base64 string of the WASM file to simulate
    #[clap(long)]
    pub source_wasm: String,

    /// The function name to simulate
    #[clap(long)]
    pub function: String,

    /// Arguments for the simulated function call
    #[clap(long, value_delimiter = ',', num_args = 1.., default_value = "")]
    pub args: Vec<String>,

    /// The network to simulate against
    #[clap(subcommand, required = true)]
    pub network: network::Network,

    /// Simulate the transaction as if it were submitted to the network
    #[clap(long)]
    pub with_submit: bool,

    /// Use the test account for the simulation
    #[clap(long)]
    pub test_account: bool,

    /// If specified, use this as the source account for the simulation instead of the test account or the default account for the network
    #[clap(long)]
    pub account: Option<String>,

    /// Session environment files to load into the simulation (JSON/YAML format)
    #[clap(long)]
    pub session_environments: Option<Vec<String>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SimulateOutput {
    pub cost: SimCost,
    pub results: Vec<SimulateInstructionResult>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SimCost {
    pub cpu_instructions: u64,
    pub mem: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SimulateInstructionResult {
    pub results: Vec<Option< soroban_sdk::token::Spec :: FunctionInputs>>,
    pub events: Vec<String>,
}

impl SimulateCmd {
    pub async fn run(&self) -> Result<SimulateOutput, Error> {
        let source_wasm = load_source_wasm_file(&self.source_wasm)?;
        let source_wasm_base64 = source_wasm_to_base64(&source_wasm);

        let network_env = self.network.clone().into()?;
        let horizon_url = get_horizon_url(&network_env);

        // Create test account if needed
        let (account, account_address) = if self.test_account {
            let test_account = create_and_fund_test_account(&horizon_url).await?;
            (test_account, test_account.address.clone())
        } else if let Some(ref acc) = self.account {
            let acc = acc.clone();
            let account = Account::from_address(&acc, &network_env)?;
            (account, acc)
        } else {
            let account =
                config::get_account_by_network(&self.network.get_network_name()?, &config::get_config()?)?;
            (account, account.address)
        };

        // Build the transaction
        let tx = soroban_sdk::Transaction::new(
            soroban_sdk::Env::default(),
            soroban_sdk::ContractFunctionCall {
                contract_id: account_address.parse()?,
                function: self.function.parse()?,
                args: self
                    .args
                    .iter()
                    .map(|arg| {
                        // Parse arguments based on their type
                        // This is a simplified version - in practice you'd need proper type handling
                        soroban_sdk::Val::from_u32(0)
                    })
                    .collect(),
            },
        );

        // Simulate the transaction
        let simulation = horizon.simulate_transaction(&tx).await?;

        Ok(SimulateOutput {
            cost: SimCost {
                cpu_instructions: simulation.cpu_instructions,
                mem: simulation.mem,
            },
            results: simulation
                .results
                .into_iter()
                .map(|r| SimulateInstructionResult {
                    results: r.results,
                    events: r.events,
                })
                .collect(),
        })
    }
}
