use super::super::locator;
use clap::command;

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
    pub fn run(&self) -> Result<(), Error> {
        println!("{}", self.config_locator.list_identities()?.join("\n"));
        Ok(())
    }
}
