use std::{fmt::Debug};

use clap::Parser;
use crate::profile::store;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    ProfileStoreError(#[from] store::Error),
}

#[derive(Parser, Debug)]
pub struct Cmd {
}

impl Cmd {
    pub fn run(&self, profiles_file: &std::path::PathBuf) -> Result<(), Error> {
        let state = store::read(profiles_file)?;
        for (name, _p) in state.profiles.iter() {
            println!("{}", name);
        }
        Ok(())
    }
}
