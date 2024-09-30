use clap::{command, Parser};

use crate::{commands::tx, xdr};

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub tx: tx::args::Args,
    /// Sequence number to bump to
    #[arg(long)]
    pub bump_to: i64,
}

impl From<&Cmd> for xdr::OperationBody {
    fn from(cmd: &Cmd) -> Self {
        xdr::OperationBody::BumpSequence(xdr::BumpSequenceOp {
            bump_to: cmd.bump_to.into(),
        })
    }
}
