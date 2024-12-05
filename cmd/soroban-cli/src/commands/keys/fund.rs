use clap::command;

use crate::config::network;

use super::public_key;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Address(#[from] public_key::Error),
    #[error(transparent)]
    Network(#[from] network::Error),
}

#[derive(Debug, clap::Parser, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub network: network::Args,
    /// Address to fund
    #[command(flatten)]
    pub address: public_key::Cmd,
}

impl Cmd {
    pub async fn run(&self) -> Result<(), Error> {
        let addr = self.address.public_key()?;
        self.network
            .get(&self.address.locator)?
            .fund_address(&addr)
            .await?;
        Ok(())
    }
}
