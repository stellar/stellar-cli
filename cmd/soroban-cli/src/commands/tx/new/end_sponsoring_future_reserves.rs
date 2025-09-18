use clap::{command, Parser};

use crate::{commands::tx, xdr};

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
    // EndSponsoringFutureReserves has no parameters
}

impl From<&Cmd> for xdr::OperationBody {
    fn from(_cmd: &Cmd) -> Self {
        xdr::OperationBody::EndSponsoringFutureReserves
    }
}
