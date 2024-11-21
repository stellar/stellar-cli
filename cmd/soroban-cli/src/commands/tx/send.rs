use crate::{print::Print, utils::transaction_hash};
use async_trait::async_trait;
use soroban_rpc::GetTransactionResponse;

use crate::{
    commands::{global, NetworkRunnable},
    config::{self, locator, network},
};

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
}

#[derive(Debug, clap::Parser, Clone)]
#[group(skip)]
/// Command to send a transaction envelope to the network
/// e.g. `cat file.txt | soroban tx send`
pub struct Cmd {
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
        let tx_env = super::xdr::tx_envelope_from_stdin()?;

        if let Ok(Ok(hash)) = super::xdr::unwrap_envelope_v1(tx_env.clone())
            .map(|tx| transaction_hash(&tx, &network.network_passphrase))
        {
            let print = Print::new(globals.map_or(false, |g| g.quiet));
            print.infoln(format!("Transaction Hash: {}", hex::encode(hash)));
        }

        Ok(client.send_transaction_polling(&tx_env).await?)
    }
}
