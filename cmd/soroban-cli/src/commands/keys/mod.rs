use clap::Parser;

pub mod add;
pub mod address;
pub mod fund;
pub mod generate;
pub mod ls;
pub mod rm;
pub mod show;
pub mod threshold_account;

#[derive(Debug, Parser)]
pub enum Cmd {
    /// Add a new identity (keypair, ledger, macOS keychain)
    Add(add::Cmd),
    /// Given an identity return its address (public key)
    Address(address::Cmd),
    /// Fund an identity on a test network
    Fund(fund::Cmd),
    /// Generate a new identity with a seed phrase, currently 12 words
    Generate(generate::Cmd),
    /// Round 1 of the SimplPedPoP protocol
    GenerateThresholdRound1(threshold_account::generate_threshold_round1::Cmd),
    /// Round 2 of the SimplPedPoP protocol
    GenerateThresholdRound2(threshold_account::generate_threshold_round2::Cmd),
    /// List identities
    Ls(ls::Cmd),
    /// Remove an identity
    Rm(rm::Cmd),
    /// Given an identity return its private key
    Show(show::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Add(#[from] add::Error),

    #[error(transparent)]
    Address(#[from] address::Error),

    #[error(transparent)]
    Fund(#[from] fund::Error),

    #[error(transparent)]
    Generate(#[from] generate::Error),

    #[error(transparent)]
    Rm(#[from] rm::Error),
    #[error(transparent)]
    Ls(#[from] ls::Error),

    #[error(transparent)]
    Show(#[from] show::Error),
}

impl Cmd {
    pub async fn run(&self) -> Result<(), Error> {
        match self {
            Cmd::Add(cmd) => cmd.run()?,
            Cmd::Address(cmd) => cmd.run()?,
            Cmd::Fund(cmd) => cmd.run().await?,
            Cmd::Generate(cmd) => cmd.run().await?,
            Cmd::Ls(cmd) => cmd.run()?,
            Cmd::Rm(cmd) => cmd.run()?,
            Cmd::Show(cmd) => cmd.run()?,
            Cmd::GenerateThresholdRound1(cmd) => cmd.run()?,
            Cmd::GenerateThresholdRound2(cmd) => cmd.run()?,
        };
        Ok(())
    }
}
