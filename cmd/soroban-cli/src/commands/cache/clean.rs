use std::fs;

use super::super::config::locator;
use crate::commands::config::data;

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
pub struct Cmd {
    /// Actions only
    #[arg(long, short = 'a')]
    pub actions: bool,
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        let dir = if self.actions {
            data::actions_dir()?
        } else {
            data::project_dir()?.data_dir().to_path_buf()
        };
        fs::remove_dir_all(dir)?;
        Ok(())
    }
}
