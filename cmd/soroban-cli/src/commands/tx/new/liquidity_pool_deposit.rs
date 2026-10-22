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
    /// Liquidity pool ID to deposit to. Accepts multiple formats:
    /// - API format with type prefix (72 chars): 000000006f2179b31311fa8064760b48942c8e166702ba0b8fbe7358c4fd570421840461
    /// - Direct hash format (64 chars): 6f2179b31311fa8064760b48942c8e166702ba0b8fbe7358c4fd570421840461
    /// - Address format (base32): LAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
    #[arg(long)]
    pub liquidity_pool_id: String,

    /// Maximum amount of the first asset to deposit, in stroops
    #[arg(long)]
    pub max_amount_a: builder::Amount,

    /// Maximum amount of the second asset to deposit, in stroops
    #[arg(long)]
    pub max_amount_b: builder::Amount,

    /// Minimum price for the first asset in terms of the second asset as "numerator:denominator" (e.g., "1:2" means 0.5)
    #[arg(long, default_value = "1:1")]
    pub min_price: String,

    /// Maximum price for the first asset in terms of the second asset as "numerator:denominator" (e.g., "1:2" means 0.5)
    #[arg(long, default_value = "1:1")]
    pub max_price: String,
}

fn parse_price(price: &str) -> Result<xdr::Price, tx::args::Error> {
    let price_parts: Vec<&str> = price.split(':').collect();
    if price_parts.len() != 2 {
        return Err(tx::args::Error::InvalidPrice(price.to_string()));
    }

    let n: i32 = price_parts[0]
        .parse()
        .map_err(|_| tx::args::Error::InvalidPrice(price.to_string()))?;
    let d: i32 = price_parts[1]
        .parse()
        .map_err(|_| tx::args::Error::InvalidPrice(price.to_string()))?;

    if d == 0 {
        return Err(tx::args::Error::InvalidPrice(
            "denominator cannot be zero".to_string(),
        ));
    }

    Ok(xdr::Price { n, d })
}

impl TryFrom<&Cmd> for xdr::OperationBody {
    type Error = tx::args::Error;
    fn try_from(
        Cmd {
            tx: _,
            op:
                Args {
                    liquidity_pool_id,
                    max_amount_a,
                    max_amount_b,
                    min_price,
                    max_price,
                },
        }: &Cmd,
    ) -> Result<Self, Self::Error> {
        let pool_id = tx::args::parse_liquidity_pool_id(liquidity_pool_id)?;

        let min_price_parsed = parse_price(min_price)?;
        let max_price_parsed = parse_price(max_price)?;

        Ok(xdr::OperationBody::LiquidityPoolDeposit(
            xdr::LiquidityPoolDepositOp {
                liquidity_pool_id: pool_id,
                max_amount_a: max_amount_a.into(),
                max_amount_b: max_amount_b.into(),
                min_price: min_price_parsed,
                max_price: max_price_parsed,
            },
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_liquidity_pool_deposit_formats() {
        let hex_id = "001041ac1d61419a62e0c0152e59f13aff7f8f65c2f04c1371ebbc662b31f4ab";
        let expected_bytes = hex::decode(hex_id).unwrap();

        let cmd = Cmd {
            tx: tx::Args {
                config: Default::default(),
                build_only: true,
            },
            op: Args {
                liquidity_pool_id: hex_id.to_string(),
                max_amount_a: "1000".parse().unwrap(),
                max_amount_b: "2000".parse().unwrap(),
                min_price: "1:2".to_string(),
                max_price: "2:1".to_string(),
            },
        };

        let op: xdr::OperationBody = (&cmd).try_into().unwrap();
        if let xdr::OperationBody::LiquidityPoolDeposit(deposit_op) = op {
            assert_eq!(deposit_op.liquidity_pool_id.0 .0.to_vec(), expected_bytes);
            assert_eq!(deposit_op.max_amount_a, 1000);
            assert_eq!(deposit_op.max_amount_b, 2000);
            assert_eq!(deposit_op.min_price, xdr::Price { n: 1, d: 2 });
            assert_eq!(deposit_op.max_price, xdr::Price { n: 2, d: 1 });
        } else {
            panic!("expected LiquidityPoolDeposit");
        }

        // Test with 00000000 prefix
        let mut cmd_prefixed = cmd.clone();
        cmd_prefixed.op.liquidity_pool_id = format!("00000000{hex_id}");
        let op_prefixed: xdr::OperationBody = (&cmd_prefixed).try_into().unwrap();
        if let xdr::OperationBody::LiquidityPoolDeposit(deposit_op) = op_prefixed {
            assert_eq!(deposit_op.liquidity_pool_id.0 .0.to_vec(), expected_bytes);
        } else {
            panic!("expected LiquidityPoolDeposit");
        }
    }
}
