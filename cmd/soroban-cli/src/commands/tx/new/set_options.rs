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
    #[arg(long)]
    pub inflation_dest: Option<builder::AccountId>,
    #[arg(long)]
    pub master_weight: Option<u8>,
    #[arg(long)]
    pub low_threshold: Option<u8>,
    #[arg(long)]
    pub med_threshold: Option<u8>,
    #[arg(long)]
    pub high_threshold: Option<u8>,
    #[arg(long)]
    pub home_domain: Option<String>,
    #[arg(long)]
    pub signer: Option<String>,

    #[arg(long, conflicts_with = "clear_required")]
    pub set_required: bool,
    #[arg(long, conflicts_with = "clear_revocable")]
    pub set_revocable: bool,
    #[arg(long, conflicts_with = "clear_immutable")]
    pub set_immutable: bool,
    #[arg(long, conflicts_with = "clear_clawback_enabled")]
    pub set_clawback_enabled: bool,
    #[arg(long)]
    pub clear_required: bool,
    #[arg(long)]
    pub clear_revocable: bool,
    #[arg(long)]
    pub clear_immutable: bool,
    #[arg(long)]
    pub clear_clawback_enabled: bool,
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
    Fqdn(#[from] fqdn::Error),
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
        let mut op = builder::ops::SetOptions::new();

        if let Some(inflation_dest) = self.inflation_dest.as_ref() {
            op = op.set_inflation_dest(inflation_dest.clone());
        };

        if let Some(master_weight) = self.master_weight {
            op = op.set_master_weight(master_weight);
        };

        if let Some(low_threshold) = self.low_threshold {
            op = op.set_low_threshold(low_threshold);
        };

        if let Some(med_threshold) = self.med_threshold {
            op = op.set_med_threshold(med_threshold);
        };

        if let Some(high_threshold) = self.high_threshold {
            op = op.set_high_threshold(high_threshold);
        };

        if let Some(home_domain) = self.home_domain.as_ref() {
            op = op.set_home_domain(&home_domain.parse()?)?;
        };

        // TODO: Signer implementation
        // if let Some(signer) = self.signer.as_ref() {
        //     let signer = signer.parse()?;
        //     op = op.set_signer(signer);
        // }

        if self.set_required {
            op = op.set_required_flag();
        };
        if self.set_revocable {
            op = op.set_revocable_flag();
        };
        if self.set_immutable {
            op = op.set_immutable_flag();
        };
        if self.set_clawback_enabled {
            op = op.set_clawback_enabled_flag();
        };
        if self.clear_required {
            op = op.clear_required_flag();
        };
        if self.clear_revocable {
            op = op.clear_revocable_flag();
        };
        if self.clear_immutable {
            op = op.clear_immutable_flag();
        };
        if self.clear_clawback_enabled {
            op = op.clear_clawback_enabled_flag();
        };

        self.tx
            .handle_tx(
                tx_build.add_operation_builder(op, None),
                &args.cloned().unwrap_or_default(),
            )
            .await?;

        Ok(TxnResult::Res(()))
    }
}
