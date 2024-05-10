use soroban_rpc::Assembled;
use soroban_sdk::xdr::{self, WriteXdr};

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
        let res = self.simulate().await?;
        println!("{}", res.transaction().to_xdr_base64(xdr::Limits::none())?);
        Ok(())
    }

    pub async fn simulate(&self) -> Result<Assembled, Error> {
        let tx = self.xdr_args.txn()?;
        let network = self.config.get_network()?;
        let client = crate::rpc::Client::new(&network.rpc_url)?;
        Ok(client.create_assembled_transaction(&tx).await?)
    }
}
