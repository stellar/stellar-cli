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

    /// The contract alias that will be displayed.
    pub alias: String,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Locator(#[from] locator::Error),

    #[error(transparent)]
    Network(#[from] network::Error),

    #[error("no contract found with alias `{alias}`")]
    NoContract { alias: String },
}

impl Cmd {
    pub fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let print = Print::new(global_args.quiet);
        let alias = &self.alias;
        let network = self.network.get(&self.config_locator)?;
        let network_passphrase = &network.network_passphrase;

        if let Some(contract) = self
            .config_locator
            .get_contract_id(&self.alias, network_passphrase)?
        {
            print.infoln(format!(
                "Contract alias '{alias}' references {contract} on network '{network_passphrase}'"
            ));

            println!("{contract}");

            Ok(())
        } else {
            Err(Error::NoContract {
                alias: alias.into(),
            })
        }
    }
}
