use std::{fs, io, path::PathBuf};

use crate::config::{data, locator};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Config(#[from] locator::Error),
    #[error(transparent)]
    Data(#[from] data::Error),
    #[error("failed to find cache entry {0}")]
    NotFound(String),
    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),
}

#[derive(Debug, clap::Parser, Clone)]
#[group(skip)]
pub struct Cmd {
    /// ID of the cache entry
    #[arg(long)]
    pub id: String,
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        let file = self.file()?;
        tracing::debug!("reading file {}", file.display());
        let mut file = fs::File::open(file).map_err(|_| Error::NotFound(self.id.clone()))?;
        let mut stdout = io::stdout();
        let _ = io::copy(&mut file, &mut stdout);
        Ok(())
    }

    pub fn file(&self) -> Result<PathBuf, Error> {
        Ok(data::actions_dir()?.join(&self.id).with_extension("json"))
    }
}
