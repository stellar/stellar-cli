use std::fmt::Debug;

use clap::Parser;

use crate::commands::{config::network, global};
use crate::config::locator;
use crate::print::Print;

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub config_locator: locator::Args,

    #[command(flatten)]
    network: network::Args,

    /// The contract alias that will be removed.
    pub alias: String,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Locator(#[from] locator::Error),

    #[error(transparent)]
    Network(#[from] network::Error),

    #[error("no contract found with alias '{alias}' for network '{network_passphrase}'")]
    NoContract {
        alias: String,
        network_passphrase: String,
    },
}

impl Cmd {
    pub fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let print = Print::new(global_args.quiet);
        let alias = &self.alias;
        let network = self.network.get(&self.config_locator)?;
        let network_passphrase = &network.network_passphrase;

        let Some(contract) = self
            .config_locator
            .get_contract_id(&self.alias, network_passphrase)?
        else {
            return Err(Error::NoContract {
                alias: alias.into(),
                network_passphrase: network_passphrase.into(),
            });
        };

        print.infoln(format!(
            "Contract alias '{alias}' references {contract} on network '{network_passphrase}'"
        ));

        self.config_locator
            .remove_contract_id(&network.network_passphrase, alias)?;

        print.checkln(format!("Contract alias '{alias}' has been removed"));

        Ok(())
    }
}
