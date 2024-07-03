use crate::{commands::global, utils::transaction_hash};
use hex;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    TxEnvelopeFromStdin(#[from] super::xdr::Error),
    #[error(transparent)]
    XdrToBase64(#[from] soroban_env_host::xdr::Error),
    #[error(transparent)]
    Config(#[from] super::super::config::Error),
}

// Command to return the transaction hash submitted to a network
/// e.g. `cat file.txt | soroban tx hash`
#[derive(Debug, clap::Parser, Clone, Default)]
#[group(skip)]
pub struct Cmd {
    #[clap(flatten)]
    pub config: super::super::config::Args,
}

impl Cmd {
    pub fn run(&self, _global_args: &global::Args) -> Result<(), Error> {
        let tx = super::xdr::unwrap_envelope_v1(super::xdr::tx_envelope_from_stdin()?)?;
        let network = &self.config.get_network()?;
        println!(
            "{}",
            hex::encode(transaction_hash(&tx, &network.network_passphrase)?)
        );
        Ok(())
    }
}
