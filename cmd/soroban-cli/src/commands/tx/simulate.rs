use crate::xdr::{self, TransactionEnvelope, WriteXdr};
use async_trait::async_trait;
use soroban_rpc::Assembled;

use crate::commands::{config, global, NetworkRunnable};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    XdrArgs(#[from] super::xdr::Error),
    #[error(transparent)]
    Config(#[from] super::super::config::Error),
    #[error(transparent)]
    Rpc(#[from] crate::rpc::Error),
    #[error(transparent)]
    Xdr(#[from] xdr::Error),
}

/// Command to simulate a transaction envelope via rpc
/// e.g. `cat file.txt | soroban tx simulate`
#[derive(Debug, clap::Parser, Clone, Default)]
#[group(skip)]
pub struct Cmd {
    #[clap(flatten)]
    pub config: super::super::config::Args,
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
        let client = crate::rpc::Client::new(&network.rpc_url)?;
        let tx = super::xdr::unwrap_envelope_v1(super::xdr::tx_envelope_from_stdin()?)?;
        Ok(client.simulate_and_assemble_transaction(&tx).await?)
    }
}
