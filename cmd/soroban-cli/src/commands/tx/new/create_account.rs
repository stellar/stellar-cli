use clap::{command, Parser};

use crate::{commands::tx, xdr};

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub tx: tx::Args,
    /// Account Id to create, e.g. `GBX...`
    #[arg(long)]
    pub destination: xdr::AccountId,
    /// Initial balance in stroops of the account, default 1 XLM
    #[arg(long, default_value = "10000000")]
    pub starting_balance: i64,
}

impl From<&Cmd> for xdr::OperationBody {
    fn from(cmd: &Cmd) -> Self {
        xdr::OperationBody::CreateAccount(xdr::CreateAccountOp {
            destination: cmd.destination.clone(),
            starting_balance: cmd.starting_balance,
        })
    }
}
