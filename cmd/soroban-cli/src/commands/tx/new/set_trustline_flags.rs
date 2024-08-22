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

#[allow(clippy::struct_excessive_bools)]
#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub tx: tx::args::Args,
    /// Account to set trustline flags for
    #[arg(long)]
    pub trustor: builder::AccountId,
    /// Asset to set trustline flags for
    #[arg(long)]
    pub asset: builder::Asset,
    #[arg(long, conflicts_with = "clear_authorize")]
    pub set_authorize: bool,
    #[arg(long, conflicts_with = "clear_authorize_to_maintain_liabilities")]
    pub set_authorize_to_maintain_liabilities: bool,
    #[arg(long, conflicts_with = "clear_trustline_clawback_enabled")]
    pub set_trustline_clawback_enabled: bool,
    #[arg(long)]
    pub clear_authorize: bool,
    #[arg(long)]
    pub clear_authorize_to_maintain_liabilities: bool,
    #[arg(long)]
    pub clear_trustline_clawback_enabled: bool,
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
        let mut op = builder::ops::SetTrustLineFlags::new(self.trustor.clone(), self.asset.clone());

        if self.set_authorize {
            op = op.set_authorized();
        }
        if self.set_authorize_to_maintain_liabilities {
            op = op.set_authorized_to_maintain_liabilities();
        }
        if self.set_trustline_clawback_enabled {
            op = op.set_trustline_clawback_enabled();
        }
        if self.clear_authorize {
            op = op.clear_authorized();
        }
        if self.clear_authorize_to_maintain_liabilities {
            op = op.clear_authorized_to_maintain_liabilities();
        }
        if self.clear_trustline_clawback_enabled {
            op = op.clear_trustline_clawback_enabled();
        }

        self.tx
            .handle_tx(
                tx_build.add_operation_builder(op, None),
                &args.cloned().unwrap_or_default(),
            )
            .await?;

        Ok(TxnResult::Res(()))
    }
}
