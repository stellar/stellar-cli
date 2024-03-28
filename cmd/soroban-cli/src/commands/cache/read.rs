use std::fs;

use super::super::config::locator;
use crate::commands::config::data;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Config(#[from] locator::Error),
    #[error(transparent)]
    Data(#[from] data::Error),
    #[error("failed to find cache entry {0}")]
    NotFound(String),
}

#[derive(Debug, clap::Parser, Clone)]
#[group(skip)]
pub struct Cmd {
    /// ULID of the cache entry
    #[arg(long, visible_alias = "id")]
    ulid: String,
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        let dir = data::actions_dir()?;
        fs::read_to_string(dir.join(&self.ulid).with_extension(".json"))
            .map_err(|_| Error::NotFound(self.ulid.clone()))?;
        Ok(())
    }
}
