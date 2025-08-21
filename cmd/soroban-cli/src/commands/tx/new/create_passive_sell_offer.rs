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
    /// Asset to sell
    #[arg(long)]
    pub selling: builder::Asset,

    /// Asset to buy
    #[arg(long)]
    pub buying: builder::Asset,

    /// Amount of selling asset to offer, in stroops. 1 stroop = 0.0000001 of the asset (e.g. 1 XLM = `10_000_000` stroops).
    #[arg(long)]
    pub amount: builder::Amount,

    /// Price of 1 unit of selling asset in terms of buying asset as "numerator:denominator" (e.g., "1:2" means 0.5)
    #[arg(long)]
    pub price: String,
}

impl TryFrom<&Cmd> for xdr::OperationBody {
    type Error = tx::args::Error;
    fn try_from(
        Cmd {
            tx,
            op:
                Args {
                    selling,
                    buying,
                    amount,
                    price,
                },
        }: &Cmd,
    ) -> Result<Self, Self::Error> {
        let price_parts: Vec<&str> = price.split(':').collect();
        if price_parts.len() != 2 {
            return Err(tx::args::Error::InvalidPrice(price.clone()));
        }

        let n: i32 = price_parts[0]
            .parse()
            .map_err(|_| tx::args::Error::InvalidPrice(price.clone()))?;
        let d: i32 = price_parts[1]
            .parse()
            .map_err(|_| tx::args::Error::InvalidPrice(price.clone()))?;

        if d == 0 {
            return Err(tx::args::Error::InvalidPrice(
                "denominator cannot be zero".to_string(),
            ));
        }

        Ok(xdr::OperationBody::CreatePassiveSellOffer(
            xdr::CreatePassiveSellOfferOp {
                selling: tx.resolve_asset(selling)?,
                buying: tx.resolve_asset(buying)?,
                amount: amount.into(),
                price: xdr::Price { n, d },
            },
        ))
    }
}
