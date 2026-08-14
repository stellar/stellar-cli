use super::args::Args;
use crate::xdr::{
    ClaimableBalanceId::ClaimableBalanceIdTypeV0, Hash, LedgerKey, LedgerKeyClaimableBalance,
};
use clap::Parser;
use hex::FromHexError;
use soroban_spec_tools::utils::padded_hex_from_str;

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    /// Claimable Balance Ids to fetch an entry for. Accepts the 64-char hex
    /// hash, the 72-char hex with type prefix returned by Horizon, or the
    /// B... address format returned by `getTransaction`
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
        let mut ledger_keys = vec![];
        self.insert_keys(&mut ledger_keys)?;
        Ok(self.args.run(ledger_keys).await?)
    }

    fn insert_keys(&self, ledger_keys: &mut Vec<LedgerKey>) -> Result<(), Error> {
        for x in &self.id {
            let hash = Hash(parse_id(x)?);
            let key = LedgerKey::ClaimableBalance(LedgerKeyClaimableBalance {
                balance_id: ClaimableBalanceIdTypeV0(hash),
            });
            ledger_keys.push(key);
        }
        Ok(())
    }
}

fn parse_id(x: &str) -> Result<[u8; 32], Error> {
    // Accept the same formats as the claimable-balance tx commands (64-char
    // hex, 72-char hex with type prefix, B... strkey), falling back to the
    // original padded-hex behavior for other input.
    if let Ok(bytes) = crate::commands::tx::new::clawback_claimable_balance::parse_balance_id(x) {
        return bytes.try_into().map_err(|_| Error::InvalidHash(x.into()));
    }
    let padded_hex = padded_hex_from_str(x, 32)?;
    padded_hex
        .try_into()
        .map_err(|_| Error::InvalidHash(x.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const HASH_HEX: &str = "6f2179b31311fa8064760b48942c8e166702ba0b8fbe7358c4fd570421840461";

    // Regression tests for https://github.com/stellar/stellar-cli/issues/2451:
    // accept the same balance-id formats as the claimable-balance tx commands.
    #[test]
    fn parses_64_char_hex() {
        assert_eq!(
            parse_id(HASH_HEX).unwrap().to_vec(),
            hex::decode(HASH_HEX).unwrap()
        );
    }

    #[test]
    fn parses_72_char_hex_with_type_prefix() {
        assert_eq!(
            parse_id(&format!("00000000{HASH_HEX}")).unwrap().to_vec(),
            hex::decode(HASH_HEX).unwrap()
        );
    }

    #[test]
    fn parses_strkey() {
        let expected = "c58728e6803ee8ea3232ea7ec5ae59e0bc8912debe7214d027e9e36fefd1d80d";
        assert_eq!(
            parse_id("BAAMLBZI42AD52HKGIZOU7WFVZM6BPEJCLPL44QU2AT6TY3P57I5QDNYIA")
                .unwrap()
                .to_vec(),
            hex::decode(expected).unwrap()
        );
    }

    #[test]
    fn short_hex_is_still_padded() {
        // Preserves the pre-existing padded-hex behavior of `--id`.
        let mut expected = vec![0u8; 31];
        expected.push(0xab);
        assert_eq!(parse_id("ab").unwrap().to_vec(), expected);
    }
}
