use clap::{command, Parser};

use crate::{commands::tx, config::address, xdr};

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
    /// Account that will be sponsored
    #[arg(long)]
    pub sponsored_id: address::UnresolvedMuxedAccount,
}

impl TryFrom<&Cmd> for xdr::OperationBody {
    type Error = tx::args::Error;
    fn try_from(
        Cmd {
            tx,
            op: Args { sponsored_id },
        }: &Cmd,
    ) -> Result<Self, Self::Error> {
        Ok(xdr::OperationBody::BeginSponsoringFutureReserves(
            xdr::BeginSponsoringFutureReservesOp {
                sponsored_id: tx.resolve_account_id(sponsored_id)?,
            },
        ))
    }
}
