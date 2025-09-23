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
    /// Liquidity pool ID to withdraw from
    #[arg(long)]
    pub liquidity_pool_id: String,

    /// Amount of pool shares to withdraw, in stroops
    #[arg(long)]
    pub amount: builder::Amount,

    /// Minimum amount of the first asset to receive, in stroops
    #[arg(long)]
    pub min_amount_a: builder::Amount,

    /// Minimum amount of the second asset to receive, in stroops
    #[arg(long)]
    pub min_amount_b: builder::Amount,
}

impl TryFrom<&Cmd> for xdr::OperationBody {
    type Error = tx::args::Error;
    fn try_from(
        Cmd {
            tx: _,
            op:
                Args {
                    liquidity_pool_id,
                    amount,
                    min_amount_a,
                    min_amount_b,
                },
        }: &Cmd,
    ) -> Result<Self, Self::Error> {
        let pool_id: xdr::PoolId = liquidity_pool_id
            .parse()
            .map_err(|_| tx::args::Error::InvalidPoolId(liquidity_pool_id.clone()))?;

        Ok(xdr::OperationBody::LiquidityPoolWithdraw(
            xdr::LiquidityPoolWithdrawOp {
                liquidity_pool_id: pool_id,
                amount: amount.into(),
                min_amount_a: min_amount_a.into(),
                min_amount_b: min_amount_b.into(),
            },
        ))
    }
}
