use clap::{command, Parser};

use crate::{
    commands::{global, tx},
    tx::builder,
    xdr,
};

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub tx: tx::args::Args,
    /// Sequence number to bump to
    #[arg(long)]
    pub bump_to: i64,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Tx(#[from] tx::args::Error),
    #[error(transparent)]
    Xdr(#[from] xdr::Error),
    #[error(transparent)]
    AssetCode(#[from] builder::asset_code::Error),
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
        xdr::OperationBody::BumpSequence(xdr::BumpSequenceOp {
            bump_to: self.bump_to.into(),
        })
    }
}
