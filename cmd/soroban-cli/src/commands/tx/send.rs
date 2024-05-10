use soroban_rpc::GetTransactionResponse;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    XdrArgs(#[from] super::xdr::Error),
    #[error(transparent)]
    Config(#[from] super::super::config::Error),
    #[error(transparent)]
    Rpc(#[from] crate::rpc::Error),
}

#[derive(Debug, clap::Parser, Clone)]
#[group(skip)]
pub struct Cmd {
    #[clap(flatten)]
    pub xdr_args: super::xdr::Args,
    #[clap(flatten)]
    pub config: super::super::config::Args,
}

impl Cmd {
    pub async fn run(&self) -> Result<(), Error> {
        let response = self.send().await?;
        println!("{response:#?}");
        Ok(())
    }

    pub async fn send(&self) -> Result<GetTransactionResponse, Error> {
        let txn_env = self.xdr_args.txn_envelope()?;
        let network = self.config.get_network()?;
        let client = crate::rpc::Client::new(&network.rpc_url)?;
        Ok(client.send_transaction(&txn_env).await?)
    }
}
