use crate::config::{data, locator};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Config(#[from] locator::Error),
    #[error(transparent)]
    Data(#[from] data::Error),
}

#[derive(Debug, clap::Parser, Clone)]
#[group(skip)]
pub struct Cmd {}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        println!("{}", data::data_local_dir()?.to_string_lossy());
        Ok(())
    }
}
