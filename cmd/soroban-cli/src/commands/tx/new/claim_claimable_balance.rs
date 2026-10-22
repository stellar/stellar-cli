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
    /// Balance ID of the claimable balance to claim. Accepts multiple formats:
    /// - API format with type prefix (72 chars): 000000006f2179b31311fa8064760b48942c8e166702ba0b8fbe7358c4fd570421840461
    /// - Direct hash format (64 chars): 6f2179b31311fa8064760b48942c8e166702ba0b8fbe7358c4fd570421840461
    /// - Address format (base32): BAAMLBZI42AD52HKGIZOU7WFVZM6BPEJCLPL44QU2AT6TY3P57I5QDNYIA
    #[arg(long)]
    pub balance_id: String,
}

impl TryFrom<&Cmd> for xdr::OperationBody {
    type Error = tx::args::Error;
    fn try_from(
        Cmd {
            tx: _,
            op: Args { balance_id },
        }: &Cmd,
    ) -> Result<Self, Self::Error> {
        let balance_id_bytes = parse_balance_id(balance_id)?;

        let mut balance_id_array = [0u8; 32];
        balance_id_array.copy_from_slice(&balance_id_bytes);

        let claimable_balance_id =
            xdr::ClaimableBalanceId::ClaimableBalanceIdTypeV0(xdr::Hash(balance_id_array));

        Ok(xdr::OperationBody::ClaimClaimableBalance(
            xdr::ClaimClaimableBalanceOp {
                balance_id: claimable_balance_id,
            },
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_balance_id_hex_parsing() {
        let balance_id = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let balance_id_bytes = hex::decode(balance_id).unwrap();
        assert_eq!(balance_id_bytes.len(), 32);

        let cmd = Cmd {
            tx: tx::Args {
                config: Default::default(),
                build_only: true,
            },
            op: Args {
                balance_id: balance_id.to_string(),
            },
        };

        let op: xdr::OperationBody = (&cmd).try_into().unwrap();
        let xdr::OperationBody::ClaimClaimableBalance(claim_op) = op else {
            panic!("expected ClaimClaimableBalance");
        };
        let xdr::ClaimableBalanceId::ClaimableBalanceIdTypeV0(hash) = claim_op.balance_id;
        assert_eq!(hash.0.to_vec(), balance_id_bytes);
    }

    #[test]
    fn test_api_format_with_prefix() {
        let api_format_id =
            "000000006f2179b31311fa8064760b48942c8e166702ba0b8fbe7358c4fd570421840461";
        let expected_hash = "6f2179b31311fa8064760b48942c8e166702ba0b8fbe7358c4fd570421840461";
        let expected_bytes = hex::decode(expected_hash).unwrap();

        let cmd = Cmd {
            tx: tx::Args {
                config: Default::default(),
                build_only: true,
            },
            op: Args {
                balance_id: api_format_id.to_string(),
            },
        };

        let op: xdr::OperationBody = (&cmd).try_into().unwrap();
        let xdr::OperationBody::ClaimClaimableBalance(claim_op) = op else {
            panic!("expected ClaimClaimableBalance");
        };
        let xdr::ClaimableBalanceId::ClaimableBalanceIdTypeV0(hash) = claim_op.balance_id;
        assert_eq!(hash.0.to_vec(), expected_bytes);
    }

    #[test]
    fn test_strkey_format() {
        let strkey_id = "BAAMLBZI42AD52HKGIZOU7WFVZM6BPEJCLPL44QU2AT6TY3P57I5QDNYIA";
        let expected_hex = "c58728e6803ee8ea3232ea7ec5ae59e0bc8912debe7214d027e9e36fefd1d80d";
        let expected_bytes = hex::decode(expected_hex).unwrap();

        let cmd = Cmd {
            tx: tx::Args {
                config: Default::default(),
                build_only: true,
            },
            op: Args {
                balance_id: strkey_id.to_string(),
            },
        };

        let op: xdr::OperationBody = (&cmd).try_into().unwrap();
        let xdr::OperationBody::ClaimClaimableBalance(claim_op) = op else {
            panic!("expected ClaimClaimableBalance");
        };
        let xdr::ClaimableBalanceId::ClaimableBalanceIdTypeV0(hash) = claim_op.balance_id;
        assert_eq!(hash.0.to_vec(), expected_bytes);
    }

    #[test]
    fn test_invalid_balance_id() {
        let cmd = Cmd {
            tx: tx::Args {
                config: Default::default(),
                build_only: true,
            },
            op: Args {
                balance_id: "0123456789abcdef".to_string(),
            },
        };
        let res: Result<xdr::OperationBody, _> = (&cmd).try_into();
        assert!(res.is_err());
    }
}
