use clap::Parser;

use crate::config;

use crate::tx::builder;
use crate::utils::contract_id_hash_from_asset;

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    /// ID of the Stellar classic asset to wrap, e.g. "native", "USDC:G...5", "USDC:alias"
    #[arg(long)]
    pub asset: builder::Asset,

    #[command(flatten)]
    pub config: config::ArgsLocatorAndNetwork,
}
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    ConfigError(#[from] config::Error),
    #[error(transparent)]
    Xdr(#[from] crate::xdr::Error),
    #[error(transparent)]
    Asset(#[from] builder::asset::Error),
}
impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        println!("{}", self.contract_address()?);
        Ok(())
    }

    pub fn contract_address(&self) -> Result<stellar_strkey::Contract, Error> {
        let network = self.config.get_network()?;
        let asset = self.asset.resolve(&self.config.locator)?;
        Ok(contract_id_hash_from_asset(
            &asset,
            &network.network_passphrase,
        ))
    }
}
