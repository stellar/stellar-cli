use std::{fs, io::ErrorKind};

use crate::config::{data, locator};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Config(#[from] locator::Error),
    #[error(transparent)]
    Data(#[from] data::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

#[derive(Debug, clap::Parser, Clone)]
#[group(skip)]
pub struct Cmd {}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        let binding = data::project_dir()?;
        let dir = binding.data_dir();
        match fs::remove_dir_all(dir) {
            Err(err) if err.kind() == ErrorKind::NotFound => (),
            r => r?,
        }
        Ok(())
    }
}
