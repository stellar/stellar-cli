use std::fmt::Debug;

use clap::Parser;
// use rand::Rng;
// use sha2::{Digest, Sha256};
// use stellar_strkey::StrkeyPublicKeyEd25519;

use crate::profile::store;

const HEADING_CONFIG: &str = "OPTIONS (CONFIG)";

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    ProfileStoreError(#[from] store::Error),
    #[error("profile already exists: {name}")]
    ProfileAlreadyExists { name: String },
}

#[derive(Default, Clone, Debug, PartialEq, Eq, clap::Args)]
#[non_exhaustive]
pub struct ProfileArgs {
    #[clap(flatten)]
    profile_config: store::Profile,

    /// File to persist profile config
    #[clap(
        long,
        parse(from_os_str),
        default_value = ".soroban/profiles.json",
        env = "SOROBAN_PROFILES_FILE",
        help_heading = HEADING_CONFIG,
    )]
    profiles_file: std::path::PathBuf,

    /// Profile to use to connect to the network
    #[clap(
        long,
        env = "SOROBAN_PROFILE",
        help_heading = HEADING_CONFIG,
    )]
    profile: Option<String>,

    /// Name of the profile, e.g. "sandbox"
    #[clap(long)]
    name: String,

    /// Overwrite any existing profile with the same name.
    #[clap(long, short = 'f')]
    force: bool,
}

impl ProfileArgs {
    pub fn set(&self) -> Result<(), Error> {
        let mut state = store::read(&self.profiles_file)?;
        for t in &mut state.profiles {
            if t.0 != self.name {
                continue;
            }
            if !self.force {
                return Err(Error::ProfileAlreadyExists {
                    name: self.name.clone(),
                });
            }
            t.1 = self.profile_config.clone();
            store::commit(&self.profiles_file, &state)?;
            return Ok(());
        }

        // Doesn't exist, add it.
        state
            .profiles
            .push((self.name.clone(), self.profile_config.clone()));
        store::commit(&self.profiles_file, &state)?;

        Ok(())
    }
}

#[derive(Parser, Debug)]
pub struct Cmd {
    #[clap(flatten)]
    profile: ProfileArgs,
}

impl Cmd {
    pub fn run(&self, _profiles_file: &std::path::PathBuf) -> Result<(), Error> {
        self.profile.set()?;
        Ok(())
    }
}
