use crate::{commands::global, print::Print};

use super::locator;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Config(#[from] locator::Error),
}

#[derive(Debug, clap::Parser, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub config_locator: locator::Args,
}

impl Cmd {
    pub fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let printer = Print::new(global_args.quiet);

        self.config_locator.unset_default_network()?;

        printer.infoln("The default network has been unset".to_string());

        Ok(())
    }
}
