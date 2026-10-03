use hex;
use std::ffi::OsString;

use crate::{
    commands::global,
    config::network,
    utils::transaction_env_hash,
    xdr::{self, TransactionEnvelope},
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    TxEnvelopeFromStdin(#[from] super::xdr::Error),
    #[error(transparent)]
    XdrToBase64(#[from] xdr::Error),
    #[error(transparent)]
    Config(#[from] network::Error),
    #[error("transaction v0 envelopes are not supported, only transaction v1 and fee bump transaction envelopes are supported")]
    TxV0Unsupported,
}

// Command to return the transaction hash submitted to a network
/// e.g. `stellar tx hash file.txt` or `cat file.txt | stellar tx hash`
#[derive(Debug, clap::Parser, Clone, Default)]
#[group(skip)]
pub struct Cmd {
    /// Base-64 transaction envelope XDR or file containing XDR to decode, or stdin if empty
    #[arg()]
    pub tx_xdr: Option<OsString>,

    #[clap(flatten)]
    pub network: network::Args,
}

impl Cmd {
    pub fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let tx_env = super::xdr::tx_envelope_from_input(&self.tx_xdr)?;
        if let TransactionEnvelope::TxV0(_) = tx_env {
            return Err(Error::TxV0Unsupported);
        }
        let network = &self.network.resolve(&global_args.locator, false)?;
        println!(
            "{}",
            hex::encode(transaction_env_hash(&tx_env, &network.network_passphrase)?)
        );
        Ok(())
    }
}
