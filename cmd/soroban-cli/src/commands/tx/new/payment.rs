use clap::{command, Parser};

use crate::{commands::tx, config::address, tx::builder, xdr};

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub tx: tx::Args,
    #[clap(flatten)]
    pub op: Args,
}

#[derive(Debug, clap::Args, Clone)]
pub struct Args {
    /// Account to send to, e.g. `GBX...`
    #[arg(long)]
    pub destination: address::Address,
    /// Asset to send, default native, e.i. XLM
    #[arg(long, default_value = "native")]
    pub asset: builder::Asset,
    /// Amount of the aforementioned asset to send. e.g. `10_000_000` (1 XLM)
    #[arg(long)]
    pub amount: builder::Amount,
}

impl TryFrom<&Cmd> for xdr::OperationBody {
    type Error = tx::args::Error;
    fn try_from(cmd: &Cmd) -> Result<Self, Self::Error> {
        Ok(xdr::OperationBody::Payment(xdr::PaymentOp {
            destination: cmd.tx.resolve_muxed_address(&cmd.op.destination)?,
            asset: cmd.op.asset.clone().into(),
            amount: cmd.op.amount.into(),
        }))
    }
}
