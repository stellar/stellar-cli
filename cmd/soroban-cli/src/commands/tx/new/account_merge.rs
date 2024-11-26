use clap::{command, Parser};

use crate::{commands::tx, config::address, xdr};

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub tx: tx::Args,
    /// Muxed Account to merge with, e.g. `GBX...`, 'MBX...' or alias
    #[arg(long)]
    pub account: address::Address,
}

impl TryFrom<&Cmd> for xdr::OperationBody {
    type Error = tx::args::Error;
    fn try_from(cmd: &Cmd) -> Result<Self, Self::Error> {
        Ok(xdr::OperationBody::AccountMerge(
            cmd.tx.reslove_muxed_address(&cmd.account)?,
        ))
    }
}
