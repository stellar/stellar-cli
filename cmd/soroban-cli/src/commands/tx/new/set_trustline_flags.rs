use clap::{command, Parser};

use soroban_sdk::xdr::{self};

use crate::{
    commands::{global, tx},
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
    /// Signifies complete authorization allowing an account to transact freely with the asset to make and receive payments and place orders.
    pub set_authorize: bool,
    #[arg(long, conflicts_with = "clear_authorize_to_maintain_liabilities")]
    /// Denotes limited authorization that allows an account to maintain current orders but not to otherwise transact with the asset.
    pub set_authorize_to_maintain_liabilities: bool,
    #[arg(long, conflicts_with = "clear_trustline_clawback_enabled")]
    /// Enables the issuing account to take back (burning) all of the asset. See our [section on Clawbacks](https://developers.stellar.org/docs/learn/encyclopedia/transactions-specialized/clawbacks)
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
        let mut set_flags = 0u32;
        let mut set_flag = |flag: xdr::TrustLineFlags| set_flags |= flag as u32;

        if self.set_authorize {
            set_flag(xdr::TrustLineFlags::AuthorizedFlag);
        };
        if self.set_authorize_to_maintain_liabilities {
            set_flag(xdr::TrustLineFlags::AuthorizedToMaintainLiabilitiesFlag);
        };
        if self.set_trustline_clawback_enabled {
            set_flag(xdr::TrustLineFlags::TrustlineClawbackEnabledFlag);
        };

        let mut clear_flags: u32 = 0;
        let mut clear_flag = |flag: xdr::TrustLineFlags| clear_flags |= flag as u32;
        if self.clear_authorize {
            clear_flag(xdr::TrustLineFlags::AuthorizedFlag);
        };
        if self.clear_authorize_to_maintain_liabilities {
            clear_flag(xdr::TrustLineFlags::AuthorizedToMaintainLiabilitiesFlag);
        };
        if self.clear_trustline_clawback_enabled {
            clear_flag(xdr::TrustLineFlags::TrustlineClawbackEnabledFlag);
        };

        xdr::OperationBody::SetTrustLineFlags(xdr::SetTrustLineFlagsOp {
            trustor: self.trustor.clone().into(),
            asset: self.asset.clone().into(),
            clear_flags,
            set_flags,
        })
    }
}
