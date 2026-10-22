use super::args::Args;
use crate::{
    commands::tx::new::clawback_claimable_balance::parse_balance_id,
    xdr::{
        ClaimableBalanceId::ClaimableBalanceIdTypeV0, Hash, LedgerKey, LedgerKeyClaimableBalance,
    },
};
use clap::Parser;
use hex::FromHexError;
use soroban_spec_tools::utils::padded_hex_from_str;

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    /// Claimable Balance Ids to fetch an entry for (64-character hex, 72-character hex with type prefix, or B... strkey format)
    #[arg(long)]
    pub id: Vec<String>,

    #[command(flatten)]
    pub args: Args,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    FromHexError(#[from] FromHexError),
    #[error("provided hash value is invalid: {0}")]
    InvalidHash(String),
    #[error(transparent)]
    Run(#[from] super::args::Error),
}

impl Cmd {
    pub async fn run(&self) -> Result<(), Error> {
        let ledger_keys = Self::parse_keys(&self.id)?;
        Ok(self.args.run(ledger_keys).await?)
    }

    pub fn parse_keys(ids: &[String]) -> Result<Vec<LedgerKey>, Error> {
        let mut ledger_keys = vec![];
        for x in ids {
            let hash_bytes: [u8; 32] = match parse_balance_id(x) {
                Ok(bytes) => bytes
                    .try_into()
                    .map_err(|_| Error::InvalidHash(x.clone()))?,
                Err(_) => {
                    let padded_hex = padded_hex_from_str(x, 32)?;
                    padded_hex
                        .try_into()
                        .map_err(|_| Error::InvalidHash(x.clone()))?
                }
            };
            let hash = Hash(hash_bytes);
            let key = LedgerKey::ClaimableBalance(LedgerKeyClaimableBalance {
                balance_id: ClaimableBalanceIdTypeV0(hash),
            });
            ledger_keys.push(key);
        }
        Ok(ledger_keys)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fetch_claimable_balance_keys_hex() {
        let ids =
            vec!["0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string()];
        let keys = Cmd::parse_keys(&ids).unwrap();
        assert_eq!(keys.len(), 1);
        if let LedgerKey::ClaimableBalance(LedgerKeyClaimableBalance {
            balance_id: ClaimableBalanceIdTypeV0(Hash(h)),
        }) = keys[0]
        {
            assert_eq!(
                hex::encode(h),
                "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
            );
        } else {
            panic!("Unexpected key variant");
        }
    }

    #[test]
    fn test_fetch_claimable_balance_keys_api_format_with_prefix() {
        let ids = vec![
            "000000000123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
        ];
        let keys = Cmd::parse_keys(&ids).unwrap();
        assert_eq!(keys.len(), 1);
        if let LedgerKey::ClaimableBalance(LedgerKeyClaimableBalance {
            balance_id: ClaimableBalanceIdTypeV0(Hash(h)),
        }) = keys[0]
        {
            assert_eq!(
                hex::encode(h),
                "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
            );
        } else {
            panic!("Unexpected key variant");
        }
    }

    #[test]
    fn test_fetch_claimable_balance_keys_strkey() {
        let strkey_id = "BAAMLBZI42AD52HKGIZOU7WFVZM6BPEJCLPL44QU2AT6TY3P57I5QDNYIA";
        let expected_hex = "c58728e6803ee8ea3232ea7ec5ae59e0bc8912debe7214d027e9e36fefd1d80d";
        let ids = vec![strkey_id.to_string()];
        let keys = Cmd::parse_keys(&ids).unwrap();
        assert_eq!(keys.len(), 1);
        if let LedgerKey::ClaimableBalance(LedgerKeyClaimableBalance {
            balance_id: ClaimableBalanceIdTypeV0(Hash(h)),
        }) = keys[0]
        {
            assert_eq!(hex::encode(h), expected_hex);
        } else {
            panic!("Unexpected key variant");
        }
    }

    #[test]
    fn test_fetch_claimable_balance_keys_padded_short_hex() {
        let ids = vec!["0123".to_string()];
        let keys = Cmd::parse_keys(&ids).unwrap();
        assert_eq!(keys.len(), 1);
        if let LedgerKey::ClaimableBalance(LedgerKeyClaimableBalance {
            balance_id: ClaimableBalanceIdTypeV0(Hash(h)),
        }) = keys[0]
        {
            assert_eq!(
                hex::encode(h),
                "0000000000000000000000000000000000000000000000000000000000000123"
            );
        } else {
            panic!("Unexpected key variant");
        }
    }

    #[test]
    fn test_fetch_claimable_balance_keys_invalid() {
        let ids = vec!["not_hex_or_strkey".to_string()];
        assert!(Cmd::parse_keys(&ids).is_err());
    }
}
