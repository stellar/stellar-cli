use clap::{command, Parser};

use crate::{commands::tx, tx::builder, xdr};

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub tx: tx::args::Args,
    /// Account to send to, e.g. `GBX...`
    #[arg(long)]
    pub destination: xdr::MuxedAccount,
    /// Asset to send, default native, e.i. XLM
    #[arg(long, default_value = "native")]
    pub asset: builder::Asset,
    /// Amount of the aforementioned asset to send.
    #[arg(long)]
    pub amount: i64,
}

impl From<&Cmd> for xdr::OperationBody {
    fn from(cmd: &Cmd) -> Self {
        xdr::OperationBody::Payment(xdr::PaymentOp {
            destination: cmd.destination.clone(),
            asset: cmd.asset.clone().into(),
            amount: cmd.amount,
        })
    }
}
