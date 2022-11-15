use std::fmt::Debug;

use clap::{Parser, Subcommand};

pub mod new;
pub mod use_profile;
pub mod current;
pub mod ls;

#[derive(Parser, Debug)]
pub struct Root {
    #[clap(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Create a new profile
    New(new::Cmd),
    /// Select the default profile to use
    Use(use_profile::Cmd),
    /// Print the current default profile
    Current(current::Cmd),
    /// List all known profiles
    Ls(ls::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    New(#[from] new::Error),
    #[error(transparent)]
    Use(#[from] use_profile::Error),
    #[error(transparent)]
    Current(#[from] current::Error),
    #[error(transparent)]
    Ls(#[from] ls::Error),
}

impl Root {
    pub fn run(&self) -> Result<(), Error> {
        match &self.cmd {
            Cmd::New(new) => new.run()?,
            Cmd::Use(use_profile) => use_profile.run()?,
            Cmd::Current(current) => current.run()?,
            Cmd::Ls(ls) => ls.run()?,
        }
        Ok(())
    }
}
