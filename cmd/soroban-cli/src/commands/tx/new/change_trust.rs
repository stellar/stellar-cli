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
    #[arg(long)]
    pub line: builder::Asset,
    /// Limit for the trust line
    #[arg(long)]
    pub limit: i64,
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
    fn build_body(&self) -> stellar_xdr::curr::OperationBody {
        let line = match self.line.0.clone() {
            xdr::Asset::CreditAlphanum4(asset) => xdr::ChangeTrustAsset::CreditAlphanum4(asset),
            xdr::Asset::CreditAlphanum12(asset) => xdr::ChangeTrustAsset::CreditAlphanum12(asset),
            xdr::Asset::Native => xdr::ChangeTrustAsset::Native,
        };
        xdr::OperationBody::ChangeTrust(xdr::ChangeTrustOp {
            line,
            limit: self.limit,
        })
    }
}
