use clap::Parser;

pub mod send;
pub mod sign;
pub mod simulate;
pub mod xdr;

use stellar_xdr::cli as xdr_cli;

#[derive(Debug, Parser)]
pub enum Cmd {
    /// Add a new identity (keypair, ledger, macOS keychain)
    Inspect(xdr_cli::Root),
    /// Given an identity return its address (public key)
    Sign(sign::Cmd),
    /// Submit a transaction to the network
    Send(send::Cmd),
    /// Simulate a transaction
    Simulate(simulate::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// An error during the simulation
    #[error(transparent)]
    Simulate(#[from] simulate::Error),
    /// An error during the inspect
    #[error(transparent)]
    Inspect(#[from] xdr_cli::Error),
    /// An error during the sign
    #[error(transparent)]
    Sign(#[from] sign::Error),
    /// An error during the send
    #[error(transparent)]
    Send(#[from] send::Error),
}

impl Cmd {
    pub async fn run(&self) -> Result<(), Error> {
        match self {
            Cmd::Inspect(cmd) => cmd.run()?,
            Cmd::Sign(cmd) => cmd.run().await?,
            Cmd::Send(cmd) => cmd.run().await?,
            Cmd::Simulate(cmd) => cmd.run().await?,
        };
        Ok(())
    }
}
