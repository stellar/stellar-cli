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

#[allow(clippy::struct_excessive_bools)]
#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub tx: tx::args::Args,
    #[arg(long)]
    /// Account of the inflation destination.
    pub inflation_dest: Option<builder::AccountId>,
    #[arg(long)]
    /// A number from 0-255 (inclusive) representing the weight of the master key. If the weight of the master key is updated to 0, it is effectively disabled.
    pub master_weight: Option<u8>,
    #[arg(long)]
    /// A number from 0-255 (inclusive) representing the threshold this account sets on all operations it performs that have [a low threshold](../../encyclopedia/security/signatures-multisig.mdx).
    pub low_threshold: Option<u8>,
    #[arg(long)]
    /// A number from 0-255 (inclusive) representing the threshold this account sets on all operations it performs that have [a medium threshold](../../encyclopedia/security/signatures-multisig.mdx).
    pub med_threshold: Option<u8>,
    #[arg(long)]
    /// A number from 0-255 (inclusive) representing the threshold this account sets on all operations it performs that have [a high threshold](../../encyclopedia/security/signatures-multisig.mdx).
    pub high_threshold: Option<u8>,
    #[arg(long)]
    /// Sets the home domain of an account. See [Federation](../../encyclopedia/network-configuration/federation.mdx).
    pub home_domain: Option<String>,
    #[arg(long)]
    /// Add, update, or remove a signer from an account.
    pub signer: Option<String>,
    #[arg(long, requires = "signer")]
    /// Signer weight is a number from 0-255 (inclusive). The signer is deleted if the weight is 0.
    pub signer_weight: Option<u8>,
    #[arg(long, conflicts_with = "clear_required")]
    /// When enabled, an issuer must approve an account before that account can hold its asset.
    /// [More info](https://developers.stellar.org/docs/tokens/control-asset-access#authorization-required-0x1)
    pub set_required: bool,
    #[arg(long, conflicts_with = "clear_revocable")]
    /// When enabled, an issuer can revoke an existing trustline’s authorization, thereby freezing the asset held by an account.
    /// [More info](https://developers.stellar.org/docs/tokens/control-asset-access#authorization-revocable-0x2)
    pub set_revocable: bool,
    #[arg(long, conflicts_with = "clear_clawback_enabled")]
    /// Enables the issuing account to take back (burning) all of the asset.
    /// [More info](https://developers.stellar.org/docs/tokens/control-asset-access#clawback-enabled-0x8)
    pub set_clawback_enabled: bool,
    #[arg(long, conflicts_with = "clear_immutable")]
    /// With this setting, none of the other authorization flags (`AUTH_REQUIRED_FLAG`, `AUTH_REVOCABLE_FLAG`) can be set, and the issuing account can’t be merged.
    /// [More info](https://developers.stellar.org/docs/tokens/control-asset-access#authorization-immutable-0x4)
    pub set_immutable: bool,
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
    Tx(#[from] tx::args::Error),
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

    pub fn op(&self) -> Result<builder::ops::SetOptions, Error> {
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

        if let (Some(signer), Some(signer_weight)) =
            (self.signer.as_ref(), self.signer_weight.as_ref())
        {
            let signer = xdr::Signer {
                key: signer.parse()?,
                weight: u32::from(*signer_weight),
            };
            op = op.set_signer(signer);
        }

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
        Ok(op)
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
                tx_build.add_operation_builder(self.op()?, self.tx.with_source_account),
                &args.cloned().unwrap_or_default(),
            )
            .await?)
    }
}
