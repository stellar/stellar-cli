use std::fmt::Debug;

use clap::{command, Parser};

use crate::commands::{config::network, global};
use crate::config::locator;
use crate::print::Print;

#[derive(Parser, Debug, Clone)]
// #[command(group(
//     clap::ArgGroup::new("wasm_src")
//         .required(true)
//         .args(&["wasm", "wasm_hash"]),
// ))]
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

    #[error("no contract found with alias `{alias}`")]
    NoContract { alias: String },
}

impl Cmd {
    #[allow(clippy::unused_async)]
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let print = Print::new(global_args.quiet);
        let alias = &self.alias;
        let network = self.network.get(&self.config_locator)?;

        print.infoln(format!(
            "Network passphrase: {passphrase}",
            passphrase = network.network_passphrase
        ));

        let contract = self
            .config_locator
            .get_contract_id(&self.alias, &network.network_passphrase)?;

        if contract.is_none() {
            return Err(Error::NoContract {
                alias: alias.into(),
            });
        };

        let contract = contract.expect("contract must be set");

        print.infoln(format!("Contract is {contract}"));

        self.config_locator
            .remove_contract_id(&network.network_passphrase, alias)?;

        print.checkln("Contract alias has been removed");

        Ok(())
    }
}
