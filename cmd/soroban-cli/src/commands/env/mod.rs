use crate::{
    commands::global,
    config::locator::{self, config, config_file},
};
use clap::Parser;

#[derive(Debug, Parser)]
pub struct Cmd {
    #[command(flatten)]
    pub config_locator: locator::Args,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Locator(#[from] locator::Error),
}

impl Cmd {
    pub fn run(&self, _global_args: &global::Args) -> Result<(), Error> {
        let config = config()?;

        println!("config_file={}", config_file()?.to_string_lossy());

        println!(
            "network={}",
            config.defaults.network.unwrap_or("(unset)".to_string())
        );

        println!(
            "identity={}",
            config.defaults.identity.unwrap_or("(unset)".to_string())
        );

        Ok(())
    }
}
