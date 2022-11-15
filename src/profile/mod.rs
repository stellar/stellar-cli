use std::fmt::Debug;

use clap::{Parser, Subcommand};
use crate::HEADING_CONFIG;

pub mod current;
pub mod ls;
pub mod new;
pub mod store;
pub mod use_profile;

#[derive(Parser, Debug)]
pub struct Root {
    /// File to persist profile config
    #[clap(
        long,
        parse(from_os_str),
        default_value = ".soroban/profiles.json",
        env = "SOROBAN_PROFILES_FILE",
        help_heading = HEADING_CONFIG,
    )]
    profiles_file: std::path::PathBuf,

    #[clap(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Print the current default profile
    Current(current::Cmd),
    /// List all known profiles
    Ls(ls::Cmd),
    /// Create a new profile
    New(new::Cmd),
    /// Select the default profile to use
    Use(use_profile::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Current(#[from] current::Error),
    #[error(transparent)]
    Ls(#[from] ls::Error),
    #[error(transparent)]
    New(#[from] new::Error),
    #[error(transparent)]
    Use(#[from] use_profile::Error),
}

impl Root {
    pub fn run(&self) -> Result<(), Error> {
        match &self.cmd {
            Cmd::Current(current) => current.run(&self.profiles_file)?,
            Cmd::Ls(ls) => ls.run(&self.profiles_file)?,
            Cmd::New(new) => new.run(&self.profiles_file)?,
            Cmd::Use(use_profile) => use_profile.run(&self.profiles_file)?,
        }
        Ok(())
    }
}
