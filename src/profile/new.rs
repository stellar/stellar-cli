use std::fmt::Debug;

use clap::Parser;
// use rand::Rng;
// use sha2::{Digest, Sha256};
// use stellar_strkey::StrkeyPublicKeyEd25519;

use crate::{profile::store, HEADING_RPC, HEADING_SANDBOX};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    ProfileStoreError(#[from] store::Error),
    #[error("profile already exists: {name}")]
    ProfileAlreadyExists { name: String },
}

#[derive(Parser, Debug)]
pub struct Cmd {
    /// Name of the profile, e.g. "sandbox"
    #[clap(long)]
    name: String,

    /// Overwrite any existing profile with the same name.
    #[clap(long, short = 'f')]
    force: bool,

    /// File to persist ledger state (if using the sandbox)
    #[clap(
        long,
        parse(from_os_str),
        default_value = ".soroban/ledger.json",
        conflicts_with = "rpc-url",
        env = "SOROBAN_LEDGER_FILE",
        help_heading = HEADING_SANDBOX,
    )]
    ledger_file: std::path::PathBuf,

    /// RPC server endpoint
    #[clap(
        long,
        requires = "network-passphrase",
        env = "SOROBAN_RPC_URL",
        help_heading = HEADING_RPC,
    )]
    rpc_url: Option<String>,
    /// Secret key to sign the transaction sent to the rpc server
    #[clap(
        long = "secret-key",
        env = "SOROBAN_SECRET_KEY",
        help_heading = HEADING_RPC,
    )]
    secret_key: Option<String>,
    /// Network passphrase to sign the transaction sent to the rpc server
    #[clap(
        long = "network-passphrase",
        env = "SOROBAN_NETWORK_PASSPHRASE",
        help_heading = HEADING_RPC,
    )]
    network_passphrase: Option<String>,
}

impl Cmd {
    pub fn run(&self, profiles_file: &std::path::PathBuf) -> Result<(), Error> {
        let mut state = store::read(profiles_file)?;

        // Generate the secret key if not provided
        let secret_key: Option<String> = self
            .secret_key
            .clone()
            .or_else(|| Some(store::generate_secret_key()));

        let p = store::Profile {
            ledger_file: Some(self.ledger_file.clone()),
            rpc_url: self.rpc_url.clone(),
            secret_key,
            network_passphrase: self.network_passphrase.clone(),
        };

        // See if it already exists
        for t in &mut state.profiles {
            if t.0 != self.name {
                continue;
            }
            if !self.force {
                return Err(Error::ProfileAlreadyExists {
                    name: self.name.clone(),
                });
            }
            t.1 = p;
            store::commit(profiles_file, &state)?;
            return Ok(());
        }

        // Doesn't exist, add it.
        state.profiles.push((self.name.clone(), p));
        store::commit(profiles_file, &state)?;
        Ok(())
    }
}
