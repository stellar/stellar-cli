use clap::{command, Parser};

use soroban_sdk::xdr::{self, Limits, WriteXdr};

use crate::{
    commands::{
        global, tx,
        txn_result::{TxnEnvelopeResult, TxnResult},
        NetworkRunnable,
    },
    config::{self},
    rpc,
    tx::builder,
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
    #[error(transparent)]
    Xdr(#[from] xdr::Error),
}

impl Cmd {
    #[allow(clippy::too_many_lines)]
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let res = self
            .run_against_rpc_server(Some(global_args), None)
            .await?
            .to_envelope();
        if let TxnEnvelopeResult::TxnEnvelope(tx) = res {
            println!("{}", tx.to_xdr_base64(Limits::none())?);
        };
        Ok(())
    }

    pub fn op(&self) -> builder::ops::ChangeTrust {
        let line = match self.line.0.clone() {
            xdr::Asset::CreditAlphanum4(asset) => xdr::ChangeTrustAsset::CreditAlphanum4(asset),
            xdr::Asset::CreditAlphanum12(asset) => xdr::ChangeTrustAsset::CreditAlphanum12(asset),
            xdr::Asset::Native => xdr::ChangeTrustAsset::Native,
        };
        builder::ops::ChangeTrust::new(line, self.limit)
    }
}

#[async_trait::async_trait]
impl NetworkRunnable for Cmd {
    type Error = Error;
    type Result = TxnResult<rpc::GetTransactionResponse>;

    async fn run_against_rpc_server(
        &self,
        args: Option<&global::Args>,
        _: Option<&config::Args>,
    ) -> Result<TxnResult<rpc::GetTransactionResponse>, Error> {
        let tx_build = self.tx.tx_builder().await?;

        Ok(self
            .tx
            .handle_tx(
                tx_build.add_operation_builder(self.op(), self.tx.with_source_account),
                &args.cloned().unwrap_or_default(),
            )
            .await?)
    }
}
