use clap::Parser;

pub mod send;
pub mod simulate;
pub mod xdr;

#[derive(Debug, Parser)]
pub enum Cmd {
    /// Submit a transaction envelope from stdin to the network
    Send(send::Cmd),
    /// Simulate a transaction envelope from stdin
    Simulate(simulate::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// An error during the simulation
    #[error(transparent)]
    Simulate(#[from] simulate::Error),
    /// An error during the send
    #[error(transparent)]
    Send(#[from] send::Error),
}

impl Cmd {
    pub async fn run(&self) -> Result<(), Error> {
        match self {
            Cmd::Send(cmd) => cmd.run().await?,
            Cmd::Simulate(cmd) => cmd.run().await?,
        };
        Ok(())
    }
}
