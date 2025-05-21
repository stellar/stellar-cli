use super::global;
mod get_transaction;
mod get_ledger_entries;

#[derive(Debug, clap::Subcommand)]
pub enum Cmd {
    GetTransaction(get_transaction::Cmd),

     #[command(subcommand)]
    GetLedgerEntries(get_ledger_entries::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    GetTransaction(#[from] get_transaction::Error),
    #[error(transparent)]
    GetLedgerEntries(#[from] get_ledger_entries::Error),
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        match self {
            Cmd::GetTransaction(cmd) => cmd.run(global_args)?,
            Cmd::GetLedgerEntries(cmd) => cmd.run(global_args).await?,
        }
        Ok(())
    }
}
