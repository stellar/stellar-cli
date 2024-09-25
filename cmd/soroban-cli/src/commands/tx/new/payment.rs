use clap::{command, Parser};

use soroban_sdk::xdr::{self};

use crate::{
    commands::{global, tx},
    config::address,
    tx::builder,
};

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub tx: tx::args::Args,
    /// Account to send to, e.g. `GBX...`
    #[arg(long)]
    pub destination: address::Address,
    /// Asset to send, default native, e.i. XLM
    #[arg(long, default_value = "native")]
    pub asset: builder::Asset,
    /// Amount of the aforementioned asset to send.
    #[arg(long)]
    pub amount: i64,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Tx(#[from] tx::args::Error),
}

impl Cmd {
    #[allow(clippy::too_many_lines)]
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        self.tx.handle_and_print(self, global_args).await?;
        Ok(())
    }
}

impl builder::Operation for Cmd {
    fn build_body(&self) -> xdr::OperationBody {
        xdr::OperationBody::Payment(xdr::PaymentOp {
            destination: self.destination.into(),
            asset: self.asset.clone().into(),
            amount: self.amount,
        })
    }
}
