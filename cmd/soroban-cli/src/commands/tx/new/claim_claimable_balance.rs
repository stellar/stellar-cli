use clap::{command, Parser};

use crate::{commands::tx, xdr};

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
    /// `BalanceID` on the `ClaimableBalanceEntry` that the source account is claiming. The balanceID can be retrieved from a successful CreateClaimableBalanceResult.
    /// For more information see: https://developers.stellar.org/docs/learn/encyclopedia/transactions-specialized/claimable-balances#create-claimable-balance
    #[arg(long)]
    pub balance_id: String,
}

impl From<&Args> for xdr::OperationBody {
    fn from(cmd: &Args) -> Self {
        let v = soroban_spec_tools::utils::padded_hex_from_str(&cmd.balance_id, 32).unwrap();
        let hash = xdr::Hash(v.try_into().unwrap());
        let balance_id = xdr::ClaimableBalanceId::ClaimableBalanceIdTypeV0(hash);
        xdr::OperationBody::ClaimClaimableBalance(xdr::ClaimClaimableBalanceOp { balance_id })
    }
}
