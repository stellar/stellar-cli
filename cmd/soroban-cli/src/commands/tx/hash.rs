use hex;
use std::ffi::OsString;

use crate::{commands::global, config::network, utils::transaction_hash};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    TxEnvelopeFromStdin(#[from] super::xdr::Error),
    #[error(transparent)]
    XdrToBase64(#[from] crate::xdr::Error),
    #[error(transparent)]
    Config(#[from] network::Error),
}

// Command to return the transaction hash submitted to a network
/// e.g. `soroban tx hash file.txt`
#[derive(Debug, clap::Parser, Clone, Default)]
#[group(skip)]
pub struct Cmd {
    /// XDR or file containing XDR to decode, or stdin if empty
    #[arg()]
    pub input: Option<OsString>,

    #[clap(flatten)]
    pub network: network::Args,
}

impl Cmd {
    pub fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let tx = super::xdr::unwrap_envelope_v1(super::xdr::tx_envelope_from_input(&self.input)?)?;
        let network = &self.network.get(&global_args.locator)?;
        println!(
            "{}",
            hex::encode(transaction_hash(&tx, &network.network_passphrase)?)
        );
        Ok(())
    }
}
