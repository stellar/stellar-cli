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

    /// Amount of buying asset to purchase, in stroops. 1 stroop = 0.0000001 of the asset (e.g. 1 XLM = `10_000_000` stroops). Use `0` to remove the offer.
    #[arg(long)]
    pub amount: builder::Amount,

    /// Price of 1 unit of buying asset in terms of selling asset as "numerator:denominator" (e.g., "1:2" means 0.5)
    #[arg(long)]
    pub price: String,

    /// Offer ID. If 0, will create new offer. Otherwise, will update existing offer.
    #[arg(long, default_value = "0")]
    pub offer_id: i64,
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
                    offer_id,
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

        Ok(xdr::OperationBody::ManageBuyOffer(xdr::ManageBuyOfferOp {
            selling: tx.resolve_asset(selling)?,
            buying: tx.resolve_asset(buying)?,
            buy_amount: amount.into(),
            price: xdr::Price { n, d },
            offer_id: *offer_id,
        }))
    }
}
