use std::io::stderr;

use clap::arg;
use soroban_rpc::GetTransactionResponse;

use crate::assembled::Assembled;
use crate::commands::tx::fetch;
use crate::commands::tx::fetch::fee::FeeTable;
use crate::commands::HEADING_RPC;

#[derive(Debug, clap::Args, Clone)]
#[group(skip)]
pub struct Args {
    /// ⚠️ Deprecated, use `--inclusion-fee`. Fee amount for transaction, in stroops. 1 stroop = 0.0000001 xlm
    #[arg(long, default_value = "100", env = "STELLAR_FEE", help_heading = HEADING_RPC)]
    pub fee: u32,
    /// Maximum fee amount for transaction inclusion, in stroops. 1 stroop = 0.0000001 xlm
    #[arg(long, default_value = "100", env = "STELLAR_INCLUSION_FEE", help_heading = HEADING_RPC)]
    pub inclusion_fee: u32,
    /// Set the fee for smart contract resource consumption, in stroops. 1 stroop = 0.0000001 xlm. Overrides the simulated resource fee
    #[arg(long, env = "STELLAR_RESOURCE_FEE", help_heading = HEADING_RPC)]
    pub resource_fee: Option<u64>,
    /// Output the cost execution to stderr
    #[arg(long = "cost", help_heading = HEADING_RPC)]
    pub cost: bool,
    /// ⚠️ Deprecated, use `--instruction_leeway` to increase instructions. Number of instructions to allocate for the transaction
    #[arg(long, help_heading = HEADING_RPC)]
    pub instructions: Option<u32>,
    /// Allow this many extra instructions when budgeting resources with transaction simulation
    #[arg(long, help_heading = HEADING_RPC)]
    pub instruction_leeway: Option<u64>,
    /// Build the transaction and only write the base64 xdr to stdout
    #[arg(long, help_heading = HEADING_RPC)]
    pub build_only: bool,
}

impl Args {
    // TODO: Remove once `--instructions` is fully removed
    pub fn apply_to_assembled_txn(&self, txn: Assembled) -> Assembled {
        if let Some(instructions) = self.instructions {
            txn.set_max_instructions(instructions)
        } else {
            txn
        }
    }

    pub fn resource_config(&self) -> Option<soroban_rpc::ResourceConfig> {
        self.instruction_leeway
            .map(|instruction_leeway| soroban_rpc::ResourceConfig { instruction_leeway })
    }

    pub fn print_cost_info(&self, res: &GetTransactionResponse) -> Result<(), fetch::Error> {
        if !self.cost {
            return Ok(());
        }

        let fee_table = FeeTable::new_from_transaction_response(res)?;

        fee_table.table().print(&mut stderr())?;

        Ok(())
    }

    // TODO: Use field directly instead of getter once `--fee` is removed
    /// Fetch the inclusion fee, prioritizing `inclusion_fee` over `fee`
    pub fn inclusion_fee(&self) -> u32 {
        if self.inclusion_fee == 100 {
            self.fee
        } else {
            self.inclusion_fee
        }
    }
}

impl Default for Args {
    fn default() -> Self {
        Self {
            fee: 100,
            inclusion_fee: 100,
            resource_fee: None,
            cost: false,
            instructions: None,
            instruction_leeway: None,
            build_only: false,
        }
    }
}
