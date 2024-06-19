use crate::commands::Error;
use clap::Parser;

pub mod aggregate_sign;
pub mod threshold_sign_round1;
pub mod threshold_sign_round2;

#[derive(Debug, Parser)]
pub enum Cmd {
    /// Round 1 of the FROST protocol
    ThresholdSignRound1(threshold_sign_round1::Cmd),
    /// Round 2 of the FROST protocol
    ThresholdSignRound2(threshold_sign_round2::Cmd),
    /// Aggregate round of the FROST protocol
    SignAggregate(aggregate_sign::Cmd),
}

impl Cmd {
    pub async fn run(&self) -> Result<(), Error> {
        match self {
            Cmd::ThresholdSignRound1(cmd) => cmd.run()?,
            Cmd::ThresholdSignRound2(cmd) => cmd.run().await?,
            Cmd::SignAggregate(cmd) => cmd.run().await?,
        };
        Ok(())
    }
}
