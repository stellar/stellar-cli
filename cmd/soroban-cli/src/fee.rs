use clap::arg;

use crate::assembled::Assembled;
use crate::xdr;

use crate::{commands::HEADING_RPC, deprecated_arg};

const DEPRECATION_MESSAGE: &str = "--sim-only is deprecated and will be removed \
in the future versions of CLI. The same functionality is offered by `tx simulate` command. To \
replicate the behaviour, run `stellar <command> --build only | stellar tx simulate`";

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
    /// Build the transaction and only write the base64 xdr to stdout
    #[arg(long, help_heading = HEADING_RPC)]
    pub build_only: bool,
    #[arg(long, visible_alias = "seq-num", help_heading = HEADING_RPC)]
    /// Optionally set the sequence number for the transaction; default is the current sequence number of the source account incremented by one
    pub sequence_number: Option<i64>,
    /// (Deprecated) simulate the transaction and only write the base64 xdr to stdout
    #[arg(
        long,
        help_heading = HEADING_RPC,
        conflicts_with = "build_only",
        display_order = 100,
        value_parser = deprecated_arg!(bool, DEPRECATION_MESSAGE))
    ]
    pub sim_only: bool,
}

impl Args {
    pub fn apply_to_assembled_txn(&self, txn: Assembled) -> Assembled {
        let txn = if let Some(instructions) = self.instructions {
            txn.set_max_instructions(instructions)
        } else {
            add_padding_to_instructions(txn)
        };
        if let Some(sequence_number) = self.sequence_number {
            txn.set_seq_num(sequence_number)
        } else {
            txn
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
            build_only: false,
            sim_only: false,
            sequence_number: None,
        }
    }
}
