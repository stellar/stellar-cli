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
    /// Get more info about the identities
    #[arg(long, short = 'l')]
    pub long: bool,
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        let res = if self.long { self.ls_l() } else { self.ls() }?.join("\n");
        println!("{res}");
        Ok(())
    }

    pub fn ls(&self) -> Result<Vec<String>, Error> {
        Ok(self.config_locator.list_identities()?)
    }

    pub fn ls_l(&self) -> Result<Vec<String>, Error> {
        Ok(self.config_locator.list_identities_long()?)
    }
}
