use clap::Parser;

use super::clawback_claimable_balance::parse_balance_id;
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
    /// Balance ID of the claimable balance to claim (64-character hex, 72-character hex with type prefix, or B... strkey format)
    #[arg(long)]
    pub balance_id: String,
}

impl TryFrom<&Args> for xdr::ClaimClaimableBalanceOp {
    type Error = tx::args::Error;
    fn try_from(Args { balance_id }: &Args) -> Result<Self, Self::Error> {
        let balance_id_bytes = parse_balance_id(balance_id)?;

        let mut balance_id_array = [0u8; 32];
        balance_id_array.copy_from_slice(&balance_id_bytes);

        let claimable_balance_id =
            xdr::ClaimableBalanceId::ClaimableBalanceIdTypeV0(xdr::Hash(balance_id_array));

        Ok(xdr::ClaimClaimableBalanceOp {
            balance_id: claimable_balance_id,
        })
    }
}

impl TryFrom<&Cmd> for xdr::OperationBody {
    type Error = tx::args::Error;
    fn try_from(cmd: &Cmd) -> Result<Self, Self::Error> {
        Ok(xdr::OperationBody::ClaimClaimableBalance(
            (&cmd.op).try_into()?,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_balance_id_hex_parsing() {
        let balance_id = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let args = Args {
            balance_id: balance_id.to_string(),
        };

        let op: xdr::ClaimClaimableBalanceOp = (&args).try_into().unwrap();
        let xdr::ClaimableBalanceId::ClaimableBalanceIdTypeV0(hash) = op.balance_id;
        assert_eq!(hex::encode(hash.0), balance_id);
    }

    #[test]
    fn test_valid_balance_id_api_format_with_prefix() {
        let balance_id = "000000000123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let expected_hash = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let args = Args {
            balance_id: balance_id.to_string(),
        };

        let op: xdr::ClaimClaimableBalanceOp = (&args).try_into().unwrap();
        let xdr::ClaimableBalanceId::ClaimableBalanceIdTypeV0(hash) = op.balance_id;
        assert_eq!(hex::encode(hash.0), expected_hash);
    }

    #[test]
    fn test_valid_balance_id_strkey_format() {
        let strkey_id = "BAAMLBZI42AD52HKGIZOU7WFVZM6BPEJCLPL44QU2AT6TY3P57I5QDNYIA";
        let expected_hex = "c58728e6803ee8ea3232ea7ec5ae59e0bc8912debe7214d027e9e36fefd1d80d";
        let args = Args {
            balance_id: strkey_id.to_string(),
        };

        let op: xdr::ClaimClaimableBalanceOp = (&args).try_into().unwrap();
        let xdr::ClaimableBalanceId::ClaimableBalanceIdTypeV0(hash) = op.balance_id;
        assert_eq!(hex::encode(hash.0), expected_hex);
    }

    #[test]
    fn test_invalid_balance_id_too_short() {
        let balance_id = "0123456789abcdef";
        let args = Args {
            balance_id: balance_id.to_string(),
        };

        let result: Result<xdr::ClaimClaimableBalanceOp, tx::args::Error> = (&args).try_into();
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_balance_id_too_long() {
        let balance_id = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef00";
        let args = Args {
            balance_id: balance_id.to_string(),
        };

        let result: Result<xdr::ClaimClaimableBalanceOp, tx::args::Error> = (&args).try_into();
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_balance_id_not_hex() {
        let balance_id = "not_hex_characters_here_not_valid_at_all_exactly_64_chars";
        let args = Args {
            balance_id: balance_id.to_string(),
        };

        let result: Result<xdr::ClaimClaimableBalanceOp, tx::args::Error> = (&args).try_into();
        assert!(result.is_err());
    }
}
