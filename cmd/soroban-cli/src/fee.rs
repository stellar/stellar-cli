use clap::arg;

use crate::assembled::Assembled;
use crate::xdr;

use crate::commands::HEADING_RPC;
use crate::print::Print;

#[derive(Debug, clap::Args, Clone)]
#[group(skip)]
pub struct Args {
    /// fee amount for transaction, in stroops. 1 stroop = 0.0000001 xlm
    #[arg(long, default_value = "100", env = "STELLAR_FEE", help_heading = HEADING_RPC)]
    pub fee: u32,
    /// Output the cost execution to stderr
    #[arg(long = "cost", help_heading = HEADING_RPC)]
    pub cost: bool,
    /// Number of instructions to simulate
    #[arg(long, help_heading = HEADING_RPC)]
    pub instructions: Option<u32>,
    /// Allow this many extra instructions when budgeting resources during transaction simulation
    #[arg(long, help_heading = HEADING_RPC)]
    pub instruction_leeway: Option<u64>,
    /// Build the transaction and only write the base64 xdr to stdout
    #[arg(long, help_heading = HEADING_RPC)]
    pub build_only: bool,
}

impl Args {
    pub fn apply_to_assembled_txn(&self, txn: Assembled) -> Assembled {
        if let Some(instructions) = self.instructions {
            txn.set_max_instructions(instructions)
        } else {
            add_padding_to_instructions(txn)
        }
    }

    pub fn resource_config(&self) -> Option<soroban_rpc::ResourceConfig> {
        self.instruction_leeway
            .map(|instruction_leeway| soroban_rpc::ResourceConfig { instruction_leeway })
    }

    pub fn print_cost_info(&self, assembled: &Assembled) {
        if !self.cost {
            return;
        }

        let print = Print::new(false);
        let txn = assembled.transaction();

        // Extract fee information from the transaction
        if let xdr::TransactionExt::V1(xdr::SorobanTransactionData { resource_fee, .. }) = &txn.ext
        {
            let total_fee = i64::from(txn.fee);
            let base_fee = total_fee - resource_fee;

            print.infoln("Cost info:");
            print.blankln(format!("Total Fee: {total_fee} stroops"));
            print.blankln(format!("Resource Fee: {resource_fee} stroops"));
            print.blankln(format!("Base Inclusion Fee: {base_fee} stroops"));
        }
    }
}

pub fn add_padding_to_instructions(txn: Assembled) -> Assembled {
    let xdr::TransactionExt::V1(xdr::SorobanTransactionData {
        resources: xdr::SorobanResources { instructions, .. },
        ..
    }) = txn.transaction().ext
    else {
        return txn;
    };
    // Start with 150%
    let instructions = (instructions.checked_mul(150 / 100)).unwrap_or(instructions);
    txn.set_max_instructions(instructions)
}

impl Default for Args {
    fn default() -> Self {
        Self {
            fee: 100,
            cost: false,
            instructions: None,
            instruction_leeway: None,
            build_only: false,
        }
    }
}
