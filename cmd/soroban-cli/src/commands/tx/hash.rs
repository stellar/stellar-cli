use hex;

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
/// e.g. `cat file.txt | soroban tx hash`
#[derive(Debug, clap::Parser, Clone, Default)]
#[group(skip)]
pub struct Cmd {
    #[clap(flatten)]
    pub network: network::Args,
}

impl Cmd {
    pub fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let tx = super::xdr::unwrap_envelope_v1(super::xdr::tx_envelope_from_stdin()?)?;
        let network = &self.network.get(&global_args.locator)?;
        println!(
            "{}",
            hex::encode(transaction_hash(&tx, &network.network_passphrase)?)
        );
        Ok(())
    }
}
