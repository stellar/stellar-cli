use clap::Parser;

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
    /// Liquidity pool ID to withdraw from. Accepts multiple formats:
    /// - API format with type prefix (72 chars): 000000006f2179b31311fa8064760b48942c8e166702ba0b8fbe7358c4fd570421840461
    /// - Direct hash format (64 chars): 6f2179b31311fa8064760b48942c8e166702ba0b8fbe7358c4fd570421840461
    /// - Address format (base32): LAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
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
        let pool_id = tx::args::parse_liquidity_pool_id(liquidity_pool_id)?;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_liquidity_pool_withdraw_formats() {
        let hex_id = "001041ac1d61419a62e0c0152e59f13aff7f8f65c2f04c1371ebbc662b31f4ab";
        let expected_bytes = hex::decode(hex_id).unwrap();

        let cmd = Cmd {
            tx: tx::Args {
                config: Default::default(),
                build_only: true,
            },
            op: Args {
                liquidity_pool_id: hex_id.to_string(),
                amount: "500".parse().unwrap(),
                min_amount_a: "100".parse().unwrap(),
                min_amount_b: "200".parse().unwrap(),
            },
        };

        let op: xdr::OperationBody = (&cmd).try_into().unwrap();
        if let xdr::OperationBody::LiquidityPoolWithdraw(withdraw_op) = op {
            assert_eq!(withdraw_op.liquidity_pool_id.0 .0.to_vec(), expected_bytes);
            assert_eq!(withdraw_op.amount, 500);
            assert_eq!(withdraw_op.min_amount_a, 100);
            assert_eq!(withdraw_op.min_amount_b, 200);
        } else {
            panic!("expected LiquidityPoolWithdraw");
        }

        // Test with 00000000 prefix
        let mut cmd_prefixed = cmd.clone();
        cmd_prefixed.op.liquidity_pool_id = format!("00000000{hex_id}");
        let op_prefixed: xdr::OperationBody = (&cmd_prefixed).try_into().unwrap();
        if let xdr::OperationBody::LiquidityPoolWithdraw(withdraw_op) = op_prefixed {
            assert_eq!(withdraw_op.liquidity_pool_id.0 .0.to_vec(), expected_bytes);
        } else {
            panic!("expected LiquidityPoolWithdraw");
        }
    }
}
