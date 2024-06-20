use crate::xdr::{self, Limits, Transaction, TransactionEnvelope, WriteXdr};

use crate::signer;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    XdrArgs(#[from] super::xdr::Error),
    #[error(transparent)]
    Signer(#[from] signer::Error),
    #[error(transparent)]
    Config(#[from] super::super::config::Error),
    #[error(transparent)]
    Xdr(#[from] xdr::Error),
}

#[derive(Debug, clap::Parser, Clone)]
#[group(skip)]
pub struct Cmd {
    #[clap(flatten)]
    pub config: super::super::config::Args,
}

impl Cmd {
    #[allow(clippy::unused_async)]
    pub async fn run(&self) -> Result<(), Error> {
        let txn = super::xdr::unwrap_envelope_v1_from_stdin()?;
        let envelope = self.sign(txn).await?;
        println!("{}", envelope.to_xdr_base64(Limits::none())?.trim());
        Ok(())
    }

    pub async fn sign(&self, tx: Transaction) -> Result<TransactionEnvelope, Error> {
        Ok(self.config.sign_with_local_key(tx).await?)
    }
}
