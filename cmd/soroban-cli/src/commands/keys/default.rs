use clap::command;

use crate::{commands::global, config::locator, print::Print};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Config(#[from] locator::Error),
}

#[derive(Debug, clap::Parser, Clone)]
#[group(skip)]
pub struct Cmd {
    /// Set the default network name.
    pub name: String,

    #[command(flatten)]
    pub config_locator: locator::Args,
}

impl Cmd {
    pub fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let printer = Print::new(global_args.quiet);
        let _ = self.config_locator.read_identity(&self.name)?;

        self.config_locator.write_default_identity(&self.name)?;

        printer.infoln(format!(
            "The default source account is set to `{}`",
            self.name,
        ));

        Ok(())
    }
}
