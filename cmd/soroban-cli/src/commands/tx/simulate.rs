use crate::{
    assembled::{simulate_and_assemble_transaction, Assembled},
    xdr::{self, TransactionEnvelope, WriteXdr},
};
use async_trait::async_trait;
use std::ffi::OsString;

use crate::commands::{config, global, NetworkRunnable};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    XdrArgs(#[from] super::xdr::Error),
    #[error(transparent)]
    Config(#[from] super::super::config::Error),
    #[error(transparent)]
    Rpc(Box<crate::rpc::Error>),
    #[error(transparent)]
    Xdr(#[from] xdr::Error),
    #[error(transparent)]
    Network(#[from] config::network::Error),
}

impl From<crate::rpc::Error> for Error {
    fn from(e: crate::rpc::Error) -> Self {
        Self::Rpc(Box::new(e))
    }
}

impl From<Box<crate::rpc::Error>> for Error {
    fn from(e: Box<crate::rpc::Error>) -> Self {
        Self::Rpc(e)
    }
}

/// Command to simulate a transaction envelope via rpc
/// e.g. `stellar tx simulate file.txt` or `cat file.txt | stellar tx simulate`
#[derive(Debug, clap::Parser, Clone, Default)]
#[group(skip)]
pub struct Cmd {
    /// Base-64 transaction envelope XDR or file containing XDR to decode, or stdin if empty
    #[arg()]
    pub tx_xdr: Option<OsString>,
    #[clap(flatten)]
    pub config: config::Args,
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let res = self
            .run_against_rpc_server(Some(global_args), Some(&self.config))
            .await?;
        let tx_env: TransactionEnvelope = res.transaction().clone().into();
        println!("{}", tx_env.to_xdr_base64(xdr::Limits::none())?);
        Ok(())
    }
}

#[async_trait]
impl NetworkRunnable for Cmd {
    type Error = Error;

    type Result = Assembled;
    async fn run_against_rpc_server(
        &self,
        _: Option<&global::Args>,
        config: Option<&config::Args>,
    ) -> Result<Self::Result, Self::Error> {
        let config = config.unwrap_or(&self.config);
        let network = config.get_network()?;
        let client = network.rpc_client()?;
        let tx = super::xdr::unwrap_envelope_v1(super::xdr::tx_envelope_from_input(&self.tx_xdr)?)?;
        let tx = simulate_and_assemble_transaction(&client, &tx).await?;
        Ok(tx)
    }
}
