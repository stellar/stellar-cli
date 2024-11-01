use std::fmt::Debug;

use clap::{command, Parser};

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

    /// The contract alias that will be used.
    pub alias: String,

    /// Overwrite the contract alias if it already exists.
    #[arg(long)]
    pub overwrite: bool,

    /// The contract id that will be associated with the alias.
    #[arg(long = "id")]
    pub contract_id: stellar_strkey::Contract,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Locator(#[from] locator::Error),

    #[error(transparent)]
    Network(#[from] network::Error),

    #[error(
        "alias '{alias}' is already referencing contract '{contract}' on network '{network_passphrase}'"
    )]
    AlreadyExist {
        alias: String,
        network_passphrase: String,
        contract: stellar_strkey::Contract,
    },
}

impl Cmd {
    pub fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let print = Print::new(global_args.quiet);
        let alias = &self.alias;
        let network = self.network.get(&self.config_locator)?;
        let network_passphrase = &network.network_passphrase;

        let contract = self
            .config_locator
            .get_contract_id(&self.alias, network_passphrase)?;

        if let Some(contract) = contract {
            if contract != self.contract_id.to_string() && !self.overwrite {
                return Err(Error::AlreadyExist {
                    alias: alias.to_string(),
                    network_passphrase: network_passphrase.to_string(),
                    contract,
                });
            }
        };

        print.infoln(format!(
            "Contract alias '{alias}' will reference {contract} on network '{network_passphrase}'",
            contract = self.contract_id
        ));

        self.config_locator.save_contract_id(
            &network.network_passphrase,
            &self.contract_id,
            alias,
        )?;

        print.checkln(format!("Contract alias '{alias}' has been added"));

        Ok(())
    }
}
