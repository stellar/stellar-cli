use std::fmt::Debug;
use std::process;

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
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Locator(#[from] locator::Error),

    #[error(transparent)]
    Network(#[from] network::Error),

    #[error(transparent)]
    PatternError(#[from] glob::PatternError),

    #[error(transparent)]
    GlobError(#[from] glob::GlobError),
}

impl Cmd {
    pub fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let print = Print::new(global_args.quiet);
        let network = self.network.get(&self.config_locator)?;
        let network_passphrase = &network.network_passphrase;
        let config_dir = self.config_locator.config_dir()?;
        let pattern = config_dir
            .join("contract-ids")
            .join("*.json")
            .to_string_lossy()
            .into_owned();

        let paths = glob::glob(&pattern)?;
        let mut found = false;

        print.infoln(format!(
            "Showing aliases for network '{network_passphrase}'"
        ));

        for path in paths {
            let path = path?;

            if let Some(alias) = path.file_stem() {
                let alias = alias.to_string_lossy().into_owned();

                if let Some(contract_id) = self
                    .config_locator
                    .get_contract_id(&alias, network_passphrase)?
                {
                    found = true;
                    println!("{alias}: {contract_id}");
                };
            }
        }

        if !found {
            print.warnln(format!(
                "No aliases defined for network '{network_passphrase}'"
            ));

            process::exit(1);
        }

        Ok(())
    }
}
