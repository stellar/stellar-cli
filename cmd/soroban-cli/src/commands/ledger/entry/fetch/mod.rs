use std::fmt::Debug;

use crate::rpc::{self};
use clap::{command, Parser};
use hex::{FromHex, FromHexError};
use soroban_spec_tools::utils::padded_hex_from_str;
use stellar_strkey::Strkey;
use stellar_strkey::{ed25519::PublicKey as Ed25519PublicKey, Contract};

pub mod account;

#[derive(Debug, Parser)]
pub enum Cmd {
    Account(account::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Account(#[from] account::Error),
}

impl Cmd {
    pub async fn run(&self) -> Result<(), Error> {
        match self {
            Cmd::Account(cmd) => cmd.run().await?,
        }
        Ok(())
    }
}
