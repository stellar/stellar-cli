use clap::arg;
use clap::builder::TypedValueParser;

use crate::assembled::Assembled;
use soroban_env_host::xdr;

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
        if let Some(instructions) = self.instructions {
            txn.set_max_instructions(instructions)
        } else {
            add_padding_to_instructions(txn)
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
        }
    }
}
