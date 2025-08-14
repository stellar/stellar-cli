use clap::{command, Parser};

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
    /// Asset to send (pay with)
    #[arg(long)]
    pub send_asset: builder::Asset,

    /// Maximum amount of send asset to deduct from sender's account, in stroops. 1 stroop = 0.0000001 of the asset (e.g. 1 XLM = `10_000_000` stroops).
    #[arg(long)]
    pub send_max: builder::Amount,

    /// Account that receives the payment
    #[arg(long)]
    pub destination: address::UnresolvedMuxedAccount,

    /// Asset that the destination will receive
    #[arg(long)]
    pub dest_asset: builder::Asset,

    /// Exact amount of destination asset that the destination account will receive, in stroops. 1 stroop = 0.0000001 of the asset.
    #[arg(long)]
    pub dest_amount: builder::Amount,

    /// List of intermediate assets for the payment path, comma-separated (up to 5 assets). Each asset should be in the format 'code:issuer' or 'native' for XLM.
    #[arg(long, value_delimiter = ',')]
    pub path: Vec<builder::Asset>,
}

impl TryFrom<&Cmd> for xdr::OperationBody {
    type Error = tx::args::Error;
    fn try_from(
        Cmd {
            tx,
            op:
                Args {
                    send_asset,
                    send_max,
                    destination,
                    dest_asset,
                    dest_amount,
                    path,
                },
        }: &Cmd,
    ) -> Result<Self, Self::Error> {
        // Validate path length (max 5 assets)
        if path.len() > 5 {
            return Err(tx::args::Error::InvalidPath(
                "Path cannot contain more than 5 assets".to_string(),
            ));
        }

        let path_assets: Result<Vec<xdr::Asset>, _> =
            path.iter().map(|asset| tx.resolve_asset(asset)).collect();
        let path_assets = path_assets?;

        let path_vec = path_assets.try_into().map_err(|_| {
            tx::args::Error::InvalidPath("Failed to convert path to VecM".to_string())
        })?;

        Ok(xdr::OperationBody::PathPaymentStrictReceive(
            xdr::PathPaymentStrictReceiveOp {
                send_asset: tx.resolve_asset(send_asset)?,
                send_max: send_max.into(),
                destination: tx.resolve_muxed_address(destination)?,
                dest_asset: tx.resolve_asset(dest_asset)?,
                dest_amount: dest_amount.into(),
                path: path_vec,
            },
        ))
    }
}
