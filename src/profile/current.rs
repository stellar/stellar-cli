use std::{array::TryFromSliceError, fmt::Debug, num::ParseIntError, rc::Rc};

use clap::Parser;

use crate::{
    snapshot, HEADING_CONFIG,
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("reading file {filepath}: {error}")]
    CannotReadConfigFile {
        filepath: std::path::PathBuf,
        error: snapshot::Error,
    },
    #[error("committing file {filepath}: {error}")]
    CannotCommitConfigFile {
        filepath: std::path::PathBuf,
        error: snapshot::Error,
    },
    #[error("cannot find profile: {name}")]
    ProfileNotFound {
        name: String,
    },
}

#[derive(Parser, Debug)]
pub struct Cmd {
    /// File to persist profile config
    #[clap(
        long,
        parse(from_os_str),
        default_value = "~/.config/soroban/config.json",
        env = "SOROBAN_CONFIG_FILE",
        help_heading = HEADING_CONFIG,
    )]
    config_file: std::path::PathBuf,
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        todo!()
    }
}
