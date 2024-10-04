use clap::{command, Parser};

use crate::{commands::tx, xdr};

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub tx: tx::Args,
    /// Muxed Account to merge with, e.g. `GBX...`, 'MBX...'
    #[arg(long)]
    pub account: xdr::MuxedAccount,
}

impl From<&Cmd> for xdr::OperationBody {
    fn from(cmd: &Cmd) -> Self {
        xdr::OperationBody::AccountMerge(cmd.account.clone())
    }
}
