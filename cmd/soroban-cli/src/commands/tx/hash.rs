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
    Config(Box<network::Error>),
}

impl From<network::Error> for Error {
    fn from(e: network::Error) -> Self {
        Self::Config(Box::new(e))
    }
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
        let tx = super::xdr::unwrap_envelope_v1(super::xdr::tx_envelope_from_input(&self.tx_xdr)?)?;
        let network = &self.network.get(&global_args.locator)?;
        println!(
            "{}",
            hex::encode(transaction_hash(&tx, &network.network_passphrase)?)
        );
        Ok(())
    }
}
