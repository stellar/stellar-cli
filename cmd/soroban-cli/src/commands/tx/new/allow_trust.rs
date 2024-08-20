use std::{fmt::Debug, str::FromStr};

use clap::{command, Parser};

use soroban_sdk::xdr::{self, Limits, WriteXdr};

use crate::{
    commands::{
        global, tx,
        txn_result::{TxnEnvelopeResult, TxnResult},
        NetworkRunnable,
    },
    config::{self, data, network, secret},
    rpc::{self},
    tx::builder,
};

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub tx: tx::args::Args,
    /// Account to merge with
    #[arg(long)]
    pub trustor: String,
    /// Asset code to allow trust for, either 4 or 12 alhanumeric characters
    #[arg(long)]
    pub asset_code: builder::AssetCode,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Rpc(#[from] rpc::Error),
    #[error(transparent)]
    Network(#[from] network::Error),
    #[error(transparent)]
    Strkey(#[from] stellar_strkey::DecodeError),
    #[error(transparent)]
    Secret(#[from] secret::Error),
    #[error(transparent)]
    Config(#[from] config::Error),
    #[error(transparent)]
    Tx(#[from] tx::args::Error),
    #[error(transparent)]
    TxBuilder(#[from] builder::Error),
    #[error(transparent)]
    Data(#[from] data::Error),
    #[error(transparent)]
    Xdr(#[from] xdr::Error),
    #[error(transparent)]
    AssetCode(#[from] builder::asset_code::Error),
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
}

#[async_trait::async_trait]
impl NetworkRunnable for Cmd {
    type Error = Error;
    type Result = TxnResult<()>;

    async fn run_against_rpc_server(
        &self,
        args: Option<&global::Args>,
        _: Option<&config::Args>,
    ) -> Result<TxnResult<()>, Error> {
        let tx_build = self.tx.tx_builder().await?;
        let account = stellar_strkey::ed25519::PublicKey::from_str(&self.trustor)?;
        let op = builder::ops::AllowTrust::new(account, self.asset_code.clone());

        self.tx
            .handle_tx(
                tx_build.add_operation_builder(op, None),
                &args.cloned().unwrap_or_default(),
            )
            .await?;

        Ok(TxnResult::Res(()))
    }
}
