use soroban_rpc::GetTransactionResponse;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    XdrArgs(#[from] super::xdr::Error),
    #[error(transparent)]
    Config(#[from] super::super::config::Error),
    #[error(transparent)]
    Rpc(#[from] crate::rpc::Error),
    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),
}

#[derive(Debug, clap::Parser, Clone)]
#[group(skip)]
/// Command to send a transaction envelope to the network
pub struct Cmd {
    #[clap(flatten)]
    pub config: super::super::config::Args,
}

impl Cmd {
    pub async fn run(&self) -> Result<(), Error> {
        let response = self.send().await?;
        println!("{}", serde_json::to_string_pretty(&response)?);
        Ok(())
    }

    pub async fn send(&self) -> Result<GetTransactionResponse, Error> {
        let txn_env = super::xdr::txn_envelope_from_stdin()?;
        let network = self.config.get_network()?;
        let client = crate::rpc::Client::new(&network.rpc_url)?;
        Ok(client.send_transaction(&txn_env).await?)
    }
}
