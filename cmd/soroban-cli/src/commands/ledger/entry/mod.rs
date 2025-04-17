pub mod get;
use clap::Parser;

#[derive(Debug, Parser)]
pub enum Cmd {
    /// Get ledger entries. This command supports every type of ledger entries supported by the
    /// RPC. Read more about RPC command here: https://developers.stellar.org/docs/data/apis/rpc/api-reference/methods/getLedgerEntries#types-of-ledgerkeys
    Get(get::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Get(#[from] get::Error),
}

impl Cmd {
    pub async fn run(&self) -> Result<(), Error> {
        match self {
            Cmd::Get(cmd) => cmd.run().await?,
        };
        Ok(())
    }
}
