use clap::Parser;

use super::Error;

pub mod generate_threshold_round1;
pub mod generate_threshold_round2;

#[derive(Debug, Parser)]
pub enum Cmd {
    /// Round 1 of the SimplPedPoP protocol
    GenerateThresholdRound1(generate_threshold_round1::Cmd),
    /// Round 2 of the SimplPedPoP protocol
    GenerateThresholdRound2(generate_threshold_round2::Cmd),
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        match self {
            Cmd::GenerateThresholdRound1(cmd) => cmd.run()?,
            Cmd::GenerateThresholdRound2(cmd) => cmd.run()?,
        };
        Ok(())
    }
}
