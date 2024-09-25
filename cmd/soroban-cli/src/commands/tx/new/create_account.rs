use clap::{command, Parser};

use crate::{
    commands::{global, tx},
    tx::builder,
    xdr,
};

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub tx: tx::args::Args,
    /// Account Id to create, e.g. `GBX...`
    #[arg(long)]
    pub destination: builder::AccountId,
    /// Initial balance in stroops of the account, default 1 XLM
    #[arg(long, default_value = "10_000_000")]
    pub starting_balance: i64,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Tx(#[from] tx::args::Error),
}

impl Cmd {
    #[allow(clippy::too_many_lines)]
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        self.tx.handle_and_print(self, global_args).await?;
        Ok(())
    }
}

impl builder::Operation for Cmd {
    fn build_body(&self) -> stellar_xdr::curr::OperationBody {
        xdr::OperationBody::CreateAccount(xdr::CreateAccountOp {
            destination: self.destination.clone().into(),
            starting_balance: self.starting_balance,
        })
    }
}
