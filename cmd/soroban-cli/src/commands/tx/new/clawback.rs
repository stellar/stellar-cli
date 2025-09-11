use clap::Parser;

use crate::{commands::tx, config::address, tx::builder, xdr};

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub tx: tx::Args,
    #[clap(flatten)]
    pub op: Args,
}

#[derive(Debug, clap::Args, Clone)]
pub struct Args {
    /// Account to clawback assets from, e.g. `GBX...`
    #[arg(long)]
    pub from: address::UnresolvedMuxedAccount,
    /// Asset to clawback
    #[arg(long)]
    pub asset: builder::Asset,
    /// Amount of the asset to clawback, in stroops. 1 stroop = 0.0000001 of the asset
    #[arg(long)]
    pub amount: builder::Amount,
}

impl TryFrom<&Cmd> for xdr::OperationBody {
    type Error = tx::args::Error;
    fn try_from(
        Cmd {
            tx,
            op:
                Args {
                    from,
                    asset,
                    amount,
                },
        }: &Cmd,
    ) -> Result<Self, Self::Error> {
        Ok(xdr::OperationBody::Clawback(xdr::ClawbackOp {
            from: tx.resolve_muxed_address(from)?,
            asset: tx.resolve_asset(asset)?,
            amount: amount.into(),
        }))
    }
}
