use std::fmt::Debug;

use crate::profile::store;
use clap::Parser;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    ProfileStoreError(#[from] store::Error),
    #[error("profile not found: {name}")]
    ProfileNotFound { name: String },
}

#[derive(Parser, Debug)]
pub struct Cmd {
    /// Name of the profile, e.g. "sandbox"
    #[clap(long)]
    name: String,
}

impl Cmd {
    pub fn run(&self, profiles_file: &std::path::PathBuf) -> Result<(), Error> {
        let mut state = store::read(profiles_file)?;
        if !state.profiles.iter().any(|(name, _)| name == &self.name) {
            return Err(Error::ProfileNotFound {
                name: self.name.clone(),
            });
        }
        state.current = self.name.clone();
        store::commit(profiles_file, &state)?;
        Ok(())
    }
}
