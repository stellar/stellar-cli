use crate::{
    assembled::{simulate_and_assemble_transaction, Assembled},
    print,
    xdr::{self, TransactionEnvelope, WriteXdr},
};
use std::ffi::OsString;

use crate::commands::{config, global};
use crate::utils::XDR_DEPTH_LIMIT;

use crate::commands::tx::fetch;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Fetch(#[from] fetch::Error),
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

    #[command(flatten)]
    pub auth_mode: crate::auth_mode::Args,

    #[command(flatten)]
    pub resources: crate::resources::Args,
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let res = self.execute(global_args, &self.config).await?;
        let tx_env: TransactionEnvelope = res.transaction().clone().into();
        self.resources.print_simulated_cost_info(&tx_env)?;
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
        let tx = super::xdr::unwrap_envelope_v1(super::xdr::tx_envelope_from_input(&self.tx_xdr)?)?;
        let resource_config = self.resources.resource_config();
        let tx = simulate_and_assemble_transaction(
            &client,
            &tx,
            resource_config,
            self.resources.resource_fee,
            self.auth_mode.to_rpc(),
        )
        .await?;
        if let Some(fee_bump_fee) = tx.fee_bump_fee() {
            print.warnln(format!("The transaction fee of {} is too large and needs to be wrapped in a fee bump transaction.", print::format_number(fee_bump_fee, 7)));
        }
        Ok(tx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tx_simulate_cost_and_leeway_flags() {
        use clap::Parser;
        let cmd = Cmd::try_parse_from(&[
            "simulate",
            "--source-account",
            "alice",
            "--cost",
            "--instruction-leeway",
            "5000",
        ])
        .unwrap();
        assert!(cmd.resources.cost);
        assert_eq!(cmd.resources.instruction_leeway, Some(5000));
    }
}
