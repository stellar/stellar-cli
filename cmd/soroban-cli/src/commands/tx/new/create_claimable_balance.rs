use clap::{command, Parser};

use crate::{commands::tx, tx::builder, xdr};

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
    /// Asset that will be held in the `ClaimableBalanceEntry` in the form `asset_code:issuing_address` or native (XLM).
    #[arg(long)]
    pub asset: builder::Asset,
    /// Amount of asset stored in the `ClaimableBalanceEntry`.
    #[arg(long)]
    pub amount: i64,
    ///List of Claimants (account address and `ClaimPredicate` pair) that can claim this `ClaimableBalanceEntry`.
    #[arg(long)]
    pub cliamants: Vec<xdr::AccountId>,
}

impl From<&Args> for xdr::OperationBody {
    fn from(cmd: &Args) -> Self {
        let asset = cmd.asset.0.clone();
        xdr::OperationBody::CreateClaimableBalance(xdr::CreateClaimableBalanceOp {
            asset,
            amount: cmd.amount,
            claimants: cmd
                .cliamants
                .iter()
                .map(|addr| {
                    xdr::Claimant::ClaimantTypeV0(xdr::ClaimantV0 {
                        destination: addr.clone(),
                        predicate: xdr::ClaimPredicate::Unconditional,
                    })
                })
                .collect::<Vec<xdr::Claimant>>()
                .try_into()
                .unwrap(),
        })
    }
}
