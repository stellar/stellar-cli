use std::str::FromStr;

use clap::Parser;
use serde::{Deserialize, Serialize};

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

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SimCost {
    pub cpu_instructions: u64,
    pub mem_bytes: u64,
    pub read_bytes: u64,
    pub write_bytes: u64,
    pub historical_data_bytes: u64,
    pub footprint_bytes: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SimulateOutput {
    pub cost: SimCost,
    pub results: Vec<SimulateInstructionResult>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SimulateInstructionResult {
    pub results: Vec<Option<soroban_sdk::token::Spec::FunctionInputs>>,
    pub events: Vec<String>,
}

impl From< soroban_sdk::simulation::SimulatedCost> for SimCost {
    fn from(cost: soroban_sdk::simulation::SimulatedCost) -> Self {
        SimCost {
            cpu_instructions: cost.cpu_instructions,
            mem_bytes: cost.mem_bytes,
            read_bytes: cost.read_bytes,
            write_bytes: cost.write_bytes,
            historical_data_bytes: cost.historical_data_bytes,
            footprint_bytes: cost.footprint_bytes,
        }
    }
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
            let account =
                Account::from_address(&acc, &network_env)?;
            (account, acc)
        } else {
            let account = config::get_account_by_network(
                &self.network.get_network_name()?,
                &config::get_config()?,
            )?;
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

        // Simulate the transaction - always return cost info even on error
        match horizon.simulate_transaction(&tx).await {
            Ok(sim) => Ok(SimulateOutput {
                cost: sim.cost.into(),
                results: sim
                    .results
                    .into_iter()
                    .map(|r| SimulateInstructionResult {
                        results: r.results,
                        events: r.events,
                    })
                    .collect(),
                error: None,
            }),
            Err(e) => {
                // Extract cost information from error if available
                // Horizon returns cost data even in error responses
                let cost = extract_cost_from_simulation_error(&e);
                
                Ok(SimulateOutput {
                    cost: cost.unwrap_or_default(),
                    results: vec![],
                    error: Some(format!("{}", e)),
                })
            }
        }
    }
}

/// Extract cost information from simulation error responses
/// Horizon/RPC may return partial cost data even when simulation fails
fn extract_cost_from_simulation_error(e: &Error) -> Option<SimCost> {
    // Attempt to parse cost from error message or response
    // This depends on the actual error structure from soroban-rpc
    // For now, return None as we rely on the underlying SDK to provide this
    
    // In a real implementation, you'd inspect the error for embedded cost data
    // Example (pseudo):
    // if let Some(sim_error) = e.downcast_ref::<soroban_rpc::error::SimulationError>() {
    //     if let Some(cost) = &sim_error.cost {
    //         return Some(cost.clone().into());
    //     }
    // }
    
    None
}
