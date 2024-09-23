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
    /// Account Id to create, e.g. `GBX...`
    #[arg(long)]
    pub destination: builder::AccountId,
    /// Initial balance in stroops of the account, default 1 XLM
    #[arg(long)]
    pub starting_balance: Option<i64>,
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

    pub fn op(&self) -> builder::ops::CreateAccount {
        builder::ops::CreateAccount::new(&self.destination, self.starting_balance)
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
