use super::super::config::locator;
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
    pub locator: locator::Args,
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        let config_dir = self.locator.config_dir()?;
        println!(
            "{}",
            config_dir.to_str().expect("Couldn't retrieve config dir")
        );

        Ok(())
    }
}
