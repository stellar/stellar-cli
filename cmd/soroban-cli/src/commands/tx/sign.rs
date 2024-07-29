use crate::{
    config::sign_with,
    xdr::{self, Limits, Transaction, TransactionEnvelope, WriteXdr},
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    XdrArgs(#[from] super::xdr::Error),
    #[error(transparent)]
    SignWith(#[from] sign_with::Error),
    #[error(transparent)]
    Xdr(#[from] xdr::Error),
}

#[derive(Debug, clap::Parser, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub sign_with: sign_with::Args,
}

impl Cmd {
    #[allow(clippy::unused_async)]
    pub async fn run(&self) -> Result<(), Error> {
        let txn_env = super::xdr::tx_envelope_from_stdin()?;
        let envelope = self
            .sign_tx(super::xdr::unwrap_envelope_v1(txn_env)?)
            .await?;
        println!("{}", envelope.to_xdr_base64(Limits::none())?.trim());
        Ok(())
    }

    pub async fn sign_tx(&self, tx: Transaction) -> Result<TransactionEnvelope, Error> {
        Ok(self.sign_with.sign(tx).await?)
    }
}
