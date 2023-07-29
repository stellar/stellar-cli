use clap::command;

use crate::commands::config::network;
use crate::rpc::{self, fund_address};

use super::config::identity::address;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Address(#[from] address::Error),
    #[error(transparent)]
    Network(#[from] network::Error),
    #[error(transparent)]
    Rpc(#[from] rpc::Error),
    #[error(transparent)]
    Strkey(#[from] stellar_strkey::DecodeError),
}

#[derive(Debug, clap::Parser, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub network: network::Args,
    /// Address to fund
    #[command(flatten)]
    pub address: address::Cmd,
}

impl Cmd {
    pub async fn run(&self) -> Result<(), Error> {
        let network = self.network.get(&self.address.locator)?;
        let addr = self.address.public_key()?;
        fund_address(&network.helper_url(&addr.to_string())).await?;
        Ok(())
    }
}
