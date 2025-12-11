use clap::{command, ValueEnum};

use crate::{
    commands::global,
    config::{locator, network},
    print::Print,
    rpc,
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Config(#[from] locator::Error),
    #[error(transparent)]
    Serde(#[from] serde_json::Error),
    #[error(transparent)]
    Network(#[from] network::Error),
    #[error(transparent)]
    Rpc(#[from] rpc::Error),
    #[error("Fee amount must be at least 100 stroops, but got {0}")]
    FeeTooSmall(u32),
    #[error("Invalid fee stats metric {0}: {1}")]
    InvalidFeeStatsResult(String, String),
}

// `clap` converts variants to kebab-case (e.g., `FeeMetric::Max` -> `max`).
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum FeeMetric {
    Max,
    Min,
    Mode,
    P10,
    P20,
    P30,
    P40,
    P50,
    P60,
    P70,
    P80,
    P90,
    P95,
    P99,
}

#[derive(Debug, clap::Parser, Clone)]
#[command(group(
    clap::ArgGroup::new("Fee Source")
    .required(true)
    .args(& ["amount", "fee_metric", "clear"]),
))]
pub struct Cmd {
    /// Set the default inclusion fee amount, in stroops. 1 stroop = 0.0000001 xlm
    #[arg(long)]
    pub amount: Option<u32>,

    /// Set the default inclusion fee based on a metric from the network's fee stats
    #[arg(long, value_enum)]
    pub fee_metric: Option<FeeMetric>,

    /// Clear the default inclusion fee setting
    #[arg(long)]
    pub clear: bool,

    #[command(flatten)]
    pub network: network::Args,

    #[command(flatten)]
    pub config_locator: locator::Args,
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let printer = Print::new(global_args.quiet);

        if std::env::var("STELLAR_INCLUSION_FEE").is_ok()
            && std::env::var("STELLAR_INCLUSION_FEE_SOURCE").is_err()
        {
            printer.warnln("Environment variable STELLAR_INCLUSION_FEE is set, which will override this default inclusion fee.");
        }

        if self.clear {
            self.config_locator.write_default_inclusion_fee(None)?;
            printer.infoln("The default inclusion fee has been cleared");
            return Ok(());
        }

        let mut inclusion_fee: u32 = 0;
        if let Some(fee_metric) = self.fee_metric {
            let network = self.network.get(&global_args.locator)?;
            let client = network.rpc_client()?;
            let fee_stats = client.get_fee_stats().await?;

            let as_string = match fee_metric {
                FeeMetric::Max => fee_stats.soroban_inclusion_fee.max,
                FeeMetric::Min => fee_stats.soroban_inclusion_fee.min,
                FeeMetric::Mode => fee_stats.soroban_inclusion_fee.mode,
                FeeMetric::P10 => fee_stats.soroban_inclusion_fee.p10,
                FeeMetric::P20 => fee_stats.soroban_inclusion_fee.p20,
                FeeMetric::P30 => fee_stats.soroban_inclusion_fee.p30,
                FeeMetric::P40 => fee_stats.soroban_inclusion_fee.p40,
                FeeMetric::P50 => fee_stats.soroban_inclusion_fee.p50,
                FeeMetric::P60 => fee_stats.soroban_inclusion_fee.p60,
                FeeMetric::P70 => fee_stats.soroban_inclusion_fee.p70,
                FeeMetric::P80 => fee_stats.soroban_inclusion_fee.p80,
                FeeMetric::P90 => fee_stats.soroban_inclusion_fee.p90,
                FeeMetric::P95 => fee_stats.soroban_inclusion_fee.p95,
                FeeMetric::P99 => fee_stats.soroban_inclusion_fee.p99,
            };
            inclusion_fee = as_string.parse::<u32>().map_err(|_| {
                Error::InvalidFeeStatsResult(format!("{fee_metric:?}"), as_string.clone())
            })?;
        } else if let Some(amount) = self.amount {
            inclusion_fee = amount;
        }

        if inclusion_fee < 100 {
            return Err(Error::FeeTooSmall(inclusion_fee));
        }

        self.config_locator
            .write_default_inclusion_fee(Some(inclusion_fee))?;

        printer.infoln(format!(
            "The default inclusion fee is set to `{inclusion_fee}`"
        ));

        Ok(())
    }
}
