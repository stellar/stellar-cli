use crate::print::Print;
use async_trait::async_trait;
use soroban_rpc::GetTransactionResponse;
use std::ffi::OsString;

use crate::{
    commands::{global, NetworkRunnable},
    config::{self, locator, network},
};

use stellar_xdr::curr;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    XdrArgs(#[from] super::xdr::Error),
    #[error(transparent)]
    Network(#[from] network::Error),
    #[error(transparent)]
    Config(#[from] config::Error),
    #[error(transparent)]
    Rpc(#[from] crate::rpc::Error),
    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),
    #[error("xdr processing error: {0}")]
    Xdr(#[from] curr::Error),
}

#[derive(Debug, clap::Parser, Clone)]
#[group(skip)]
/// Command to send a transaction envelope to the network
/// e.g. `stellar tx send file.txt` or `cat file.txt | stellar tx send`
pub struct Cmd {
    /// Base-64 transaction envelope XDR or file containing XDR to decode, or stdin if empty
    #[arg()]
    pub tx_xdr: Option<OsString>,
    #[clap(flatten)]
    pub network: network::Args,
    #[clap(flatten)]
    pub locator: locator::Args,
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let response = self.run_against_rpc_server(Some(global_args), None).await?;
        println!("{}", serde_json::to_string_pretty(&response)?);
        Ok(())
    }
}

#[async_trait]
impl NetworkRunnable for Cmd {
    type Error = Error;

    type Result = GetTransactionResponse;
    async fn run_against_rpc_server(
        &self,
        globals: Option<&global::Args>,
        config: Option<&config::Args>,
    ) -> Result<Self::Result, Self::Error> {
        let network = if let Some(config) = config {
            config.get_network()?
        } else {
            self.network.get(&self.locator)?
        };
        let client = network.rpc_client()?;
        let tx_env = super::xdr::tx_envelope_from_input(&self.tx_xdr)?;

        if let Ok(txn) = super::xdr::unwrap_envelope_v1(tx_env.clone()) {
            let print = Print::new(globals.map_or(false, |g| g.quiet));
            print.log_transaction(&txn, &network, true)?;
        }

        Ok(client.send_transaction_polling(&tx_env).await?)
    }
}
