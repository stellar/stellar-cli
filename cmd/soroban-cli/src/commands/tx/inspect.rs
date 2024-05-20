use soroban_rpc::GetTransactionResponse;
use stellar_xdr::cli as xdr_cli;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    XdrCli(#[from] xdr_cli::Error),
}

#[derive(Debug, clap::Parser, Clone)]
#[group(skip)]
pub struct Cmd {
    #[clap(flatten)]
    pub xdr_args: xdr_cli::Root,
    
}

impl Cmd {
    pub async fn run(&self) -> Result<(), Error> {
        let response = self.send().await?;
        println!("{}", serde_json::to_string_pretty(&response)?);
        Ok(())
    }

    pub async fn send(&self) -> Result<GetTransactionResponse, Error> {
        let txn_env = self.xdr_args.txn_envelope()?;
        let network = self.config.get_network()?;
        let client = crate::rpc::Client::new(&network.rpc_url)?;
        Ok(client.send_transaction(&txn_env).await?)
    }
}
