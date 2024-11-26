use clap::{command, Parser};

use crate::{commands::tx, config::address, xdr};

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub tx: tx::Args,
    /// Account Id to create, e.g. `GBX...`
    #[arg(long)]
    pub destination: address::Address,
    /// Initial balance in stroops of the account, default 1 XLM
    #[arg(long, default_value = "10_000_000")]
    pub starting_balance: builder::Amount,
}

impl TryFrom<&Cmd> for xdr::OperationBody {
    type Error = tx::args::Error;
    fn try_from(cmd: &Cmd) -> Result<Self, Self::Error> {
        Ok(xdr::OperationBody::CreateAccount(xdr::CreateAccountOp {
            destination: cmd.tx.reslove_account_id(&cmd.destination)?,
            starting_balance: cmd.starting_balance,
        }))
    }
}
