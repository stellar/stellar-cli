use std::io::stderr;

use soroban_rpc::GetTransactionResponse;

use crate::assembled::Assembled;
use crate::commands::tx::fetch;
use crate::commands::tx::fetch::fee::FeeTable;
use crate::commands::HEADING_RPC;

#[derive(Debug, clap::Args, Clone, Default)]
#[group(skip)]
pub struct Args {
    /// Set the fee for smart contract resource consumption, in stroops. 1 stroop = 0.0000001 xlm. Overrides the simulated resource fee
    #[arg(long, env = "STELLAR_RESOURCE_FEE", value_parser = clap::value_parser!(i64).range(0..i64::MAX), help_heading = HEADING_RPC)]
    pub resource_fee: Option<i64>,
    /// ⚠️ Deprecated, use `--instruction-leeway` to increase instructions. Number of instructions to allocate for the transaction
    #[arg(long, help_heading = HEADING_RPC)]
    pub instructions: Option<u32>,
    /// Allow this many extra instructions when budgeting resources with transaction simulation
    #[arg(long, help_heading = HEADING_RPC)]
    pub instruction_leeway: Option<u64>,
    /// Output the cost execution to stderr
    #[arg(long, help_heading = HEADING_RPC)]
    pub cost: bool,
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
}
