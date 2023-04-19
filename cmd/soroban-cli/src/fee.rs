use clap::arg;

use crate::commands::HEADING_RPC;

#[derive(Debug, clap::Args, Clone)]
#[group(skip)]
pub struct Args {
    /// fee amount for transaction, in stroops. 1 stroop = 0.0000001 xlm
    #[arg(long, default_value = "100", env = "SOROBAN_FEE", help_heading = HEADING_RPC)]
    pub fee: u32,
}

impl Default for Args {
    fn default() -> Self {
        Self { fee: 100 }
    }
}
