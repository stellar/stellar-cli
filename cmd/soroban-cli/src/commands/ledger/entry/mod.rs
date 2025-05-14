use clap::Parser;
pub mod fetch;

#[derive(Debug, Parser)]
pub enum Cmd {
    /// Fetch ledger entries. This command supports all types of ledger entries supported by the RPC.
    /// Read more about the RPC command here: https://developers.stellar.org/docs/data/apis/rpc/api-reference/methods/getLedgerEntries#types-of-ledgerkeys
    #[command(subcommand)]
    Fetch(fetch::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Fetch(#[from] fetch::Error),
}

impl Cmd {
    pub async fn run(&self) -> Result<(), Error> {
        match self {
            Cmd::Fetch(cmd) => cmd.run().await?,
        };
        Ok(())
    }
}