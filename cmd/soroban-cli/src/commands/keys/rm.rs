use clap::command;

use crate::commands::global;

use super::super::config::locator;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Locator(#[from] locator::Error),
}

#[derive(Debug, clap::Parser, Clone)]
#[group(skip)]
pub struct Cmd {
    /// Identity to remove
    pub name: String,

    #[command(flatten)]
    pub config: locator::Args,
}

impl Cmd {
    pub fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        Ok(self.config.remove_identity(&self.name, global_args)?)
    }
}
