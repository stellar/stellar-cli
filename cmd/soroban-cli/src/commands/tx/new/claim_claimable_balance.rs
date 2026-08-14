use clap::Parser;

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
        Ok(xdr::OperationBody::ClaimClaimableBalance(
            xdr::ClaimClaimableBalanceOp {
                balance_id: claimable_balance_id(balance_id)?,
            },
        ))
    }
}

fn claimable_balance_id(balance_id: &str) -> Result<xdr::ClaimableBalanceId, tx::args::Error> {
    let balance_id_bytes = super::clawback_claimable_balance::parse_balance_id(balance_id)?;

    let mut balance_id_array = [0u8; 32];
    balance_id_array.copy_from_slice(&balance_id_bytes);

    Ok(xdr::ClaimableBalanceId::ClaimableBalanceIdTypeV0(
        xdr::Hash(balance_id_array),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const HASH_HEX: &str = "6f2179b31311fa8064760b48942c8e166702ba0b8fbe7358c4fd570421840461";

    fn hash_of(id: &xdr::ClaimableBalanceId) -> Vec<u8> {
        let xdr::ClaimableBalanceId::ClaimableBalanceIdTypeV0(hash) = id;
        hash.0.to_vec()
    }

    #[test]
    fn accepts_64_char_hex() {
        let id = claimable_balance_id(HASH_HEX).unwrap();
        assert_eq!(hash_of(&id), hex::decode(HASH_HEX).unwrap());
    }

    // Regression tests for https://github.com/stellar/stellar-cli/issues/2451:
    // the formats returned by Horizon (72-char hex with type prefix) and by
    // `getTransaction` (B... strkey) must be accepted, consistent with
    // `clawback-claimable-balance`.
    #[test]
    fn accepts_72_char_hex_with_type_prefix() {
        let id = claimable_balance_id(&format!("00000000{HASH_HEX}")).unwrap();
        assert_eq!(hash_of(&id), hex::decode(HASH_HEX).unwrap());
    }

    #[test]
    fn accepts_strkey() {
        let strkey = "BAAMLBZI42AD52HKGIZOU7WFVZM6BPEJCLPL44QU2AT6TY3P57I5QDNYIA";
        let expected = "c58728e6803ee8ea3232ea7ec5ae59e0bc8912debe7214d027e9e36fefd1d80d";
        let id = claimable_balance_id(strkey).unwrap();
        assert_eq!(hash_of(&id), hex::decode(expected).unwrap());
    }

    #[test]
    fn rejects_invalid_ids() {
        // Too short, too long, and non-hex input.
        assert!(claimable_balance_id("0123456789abcdef").is_err());
        assert!(claimable_balance_id(&format!("{HASH_HEX}00")).is_err());
        assert!(
            claimable_balance_id("not_hex_characters_here_not_valid_at_all_exactly_64_chars")
                .is_err()
        );
    }
}
