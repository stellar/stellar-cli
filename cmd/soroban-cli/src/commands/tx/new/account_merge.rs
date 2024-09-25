use clap::{command, Parser};

use soroban_sdk::xdr::{self};

use crate::{
    commands::{global, tx},
    config::address,
    tx::builder::{self},
};

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub tx: tx::args::Args,
    /// Account to merge with, e.g. `GBX...`
    #[arg(long)]
    pub account: address::Address,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Tx(#[from] tx::args::Error),
    #[error(transparent)]
    Xdr(#[from] xdr::Error),
    #[error(transparent)]
    Address(#[from] address::Error),
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
        xdr::OperationBody::AccountMerge(self.account.into())
    }
}
