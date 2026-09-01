use crate::{
    assembled::{simulate_and_assemble_transaction, Assembled},
    print,
    xdr::{self, TransactionEnvelope, WriteXdr},
};
use std::ffi::OsString;

use crate::commands::{config, global};
use crate::utils::XDR_DEPTH_LIMIT;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    XdrArgs(#[from] super::xdr::Error),
    #[error(transparent)]
    Config(#[from] super::super::config::Error),
    #[error(transparent)]
    Rpc(#[from] crate::rpc::Error),
    #[error(transparent)]
    Xdr(#[from] xdr::Error),
    #[error(transparent)]
    Network(#[from] config::network::Error),
}

/// Command to simulate a transaction envelope via rpc
/// e.g. `stellar tx simulate file.txt` or `cat file.txt | stellar tx simulate`
#[derive(Debug, clap::Parser, Clone, Default)]
#[group(skip)]
pub struct Cmd {
    /// Base-64 transaction envelope XDR or file containing XDR to decode, or stdin if empty
    #[arg()]
    pub tx_xdr: Option<OsString>,

    #[clap(flatten)]
    pub config: config::Args,

    /// Allow this many extra instructions when budgeting resources during transaction simulation
    #[arg(long)]
    pub instruction_leeway: Option<u64>,

    #[command(flatten)]
    pub auth_mode: crate::auth_mode::Args,
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let res = self.execute(global_args, &self.config).await?;
        let tx_env: TransactionEnvelope = res.transaction().clone().into();
        println!(
            "{}",
            tx_env.to_xdr_base64(xdr::Limits::depth(XDR_DEPTH_LIMIT))?
        );
        Ok(())
    }

    pub async fn execute(
        &self,
        global_args: &global::Args,
        config: &config::Args,
    ) -> Result<Assembled, Error> {
        let print = print::Print::new(global_args.quiet);
        let network = config.get_network()?;
        let client = network.rpc_client()?;
        let tx_env = super::xdr::tx_envelope_from_input(&self.tx_xdr)?;
        // Simulation rewrites the fee and Soroban transaction data, so any
        // signature on the incoming envelope is invalid on the simulated
        // result. They were silently dropped before; say so, since the correct
        // order (simulate, then sign) is the actual fix on the user's side.
        // Warn only for v1 envelopes: v0 and fee-bump envelopes are rejected
        // below by unwrap_envelope_v1 with their signatures intact, so nothing
        // is discarded for them.
        let discarded = if matches!(tx_env, TransactionEnvelope::Tx(_)) {
            super::xdr::signature_count(&tx_env)
        } else {
            0
        };
        if discarded > 0 {
            print.warnln(format!(
                "Discarding {discarded} existing signature(s): simulation changes the \
                 transaction's fee and resources, which invalidates prior signatures. \
                 Simulate first, then sign the result."
            ));
        }
        let tx = super::xdr::unwrap_envelope_v1(tx_env)?;
        let resource_config = self
            .instruction_leeway
            .map(|instruction_leeway| soroban_rpc::ResourceConfig { instruction_leeway });
        let tx = simulate_and_assemble_transaction(
            &client,
            &tx,
            resource_config,
            None,
            self.auth_mode.to_rpc(),
        )
        .await?;
        if let Some(fee_bump_fee) = tx.fee_bump_fee() {
            print.warnln(format!("The transaction fee of {} is too large and needs to be wrapped in a fee bump transaction.", print::format_number(fee_bump_fee, 7)));
        }
        Ok(tx)
    }
}
