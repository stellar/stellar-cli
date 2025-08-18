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
    /// Balance ID of the claimable balance to claim (64-character hex string)
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
        let balance_id_bytes =
            hex::decode(balance_id).map_err(|_| tx::args::Error::InvalidHex {
                name: "balance-id".to_string(),
                hex: balance_id.clone(),
            })?;

        if balance_id_bytes.len() != 32 {
            return Err(tx::args::Error::InvalidHex {
                name: "balance-id".to_string(),
                hex: balance_id.clone(),
            });
        }

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

        let mut balance_id_array = [0u8; 32];
        balance_id_array.copy_from_slice(&balance_id_bytes);

        let claimable_balance_id =
            xdr::ClaimableBalanceId::ClaimableBalanceIdTypeV0(xdr::Hash(balance_id_array));

        let op = xdr::ClaimClaimableBalanceOp {
            balance_id: claimable_balance_id,
        };

        let xdr::ClaimableBalanceId::ClaimableBalanceIdTypeV0(hash) = op.balance_id;
        assert_eq!(hash.0.to_vec(), balance_id_bytes);
    }

    #[test]
    fn test_invalid_balance_id_too_short() {
        let balance_id = "0123456789abcdef";
        let balance_id_bytes = hex::decode(balance_id).unwrap();
        assert_ne!(balance_id_bytes.len(), 32);
    }

    #[test]
    fn test_invalid_balance_id_too_long() {
        let balance_id = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef00";
        let balance_id_bytes = hex::decode(balance_id).unwrap();
        assert_ne!(balance_id_bytes.len(), 32);
    }

    #[test]
    fn test_invalid_balance_id_not_hex() {
        let balance_id = "not_hex_characters_here_not_valid_at_all_exactly_64_chars";
        let result = hex::decode(balance_id);
        assert!(result.is_err());
    }
}
