use crate::{
    config::{locator, network, sign_with},
    xdr::{self, Limits, TransactionEnvelope, WriteXdr},
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    XdrArgs(#[from] super::xdr::Error),
    #[error(transparent)]
    Network(#[from] network::Error),
    #[error(transparent)]
    Locator(#[from] locator::Error),
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
    #[command(flatten)]
    pub network: network::Args,
    #[command(flatten)]
    pub locator: locator::Args,
}

impl Cmd {
    #[allow(clippy::unused_async)]
    pub async fn run(&self) -> Result<(), Error> {
        let txn_env = super::xdr::tx_envelope_from_stdin()?;
        if self.sign_with.sign_with_lab {
            return self.sign_tx_env_with_lab(txn_env).await;
        } else {
            let envelope = self.sign_tx_env(txn_env).await?;
            println!("{}", envelope.to_xdr_base64(Limits::none())?.trim());
        }
        Ok(())
    }

    pub async fn sign_tx_env(&self, tx: TransactionEnvelope) -> Result<TransactionEnvelope, Error> {
        Ok(self
            .sign_with
            .sign_txn_env(tx, &self.locator, &self.network.get(&self.locator)?)
            .await?)
    }

    pub async fn sign_tx_env_with_lab(&self, tx: TransactionEnvelope) -> Result<(), Error> {
        Ok(self.sign_with.sign_tx_env_with_lab().await?)
    }
}
