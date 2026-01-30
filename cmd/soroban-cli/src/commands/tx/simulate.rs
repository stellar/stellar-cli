use crate::{
    assembled::{simulate_and_assemble_transaction, Assembled},
    xdr::{self, TransactionEnvelope, WriteXdr},
};
use std::ffi::OsString;

use crate::commands::{config, global};

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
}

impl Cmd {
    pub async fn run(&self, _global_args: &global::Args) -> Result<(), Error> {
        let res = self.execute(&self.config).await?;
        let tx_env: TransactionEnvelope = res.transaction().clone().into();
        println!("{}", tx_env.to_xdr_base64(xdr::Limits::none())?);
        Ok(())
    }

    pub async fn execute(&self, config: &config::Args) -> Result<Assembled, Error> {
        let network = config.get_network()?;
        let client = network.rpc_client()?;
        let tx = super::xdr::unwrap_envelope_v1(super::xdr::tx_envelope_from_input(&self.tx_xdr)?)?;
        let resource_config = self
            .instruction_leeway
            .map(|instruction_leeway| soroban_rpc::ResourceConfig { instruction_leeway });
        let tx = simulate_and_assemble_transaction(&client, &tx, resource_config, None).await?;
        Ok(tx)
    }
}
