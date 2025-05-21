use super::args::Args;
use crate::xdr::{
    ClaimableBalanceId::ClaimableBalanceIdTypeV0, Hash, LedgerKey, LedgerKeyClaimableBalance,
};
use clap::{command, Parser};
use hex::FromHexError;
use soroban_spec_tools::utils::padded_hex_from_str;

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    /// Claimable Balance Ids to fetch an entry for
    pub ids: Vec<String>,

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
        for x in &self.ids {
            let padded_hex = padded_hex_from_str(x, 32)?;
            let hash_bytes: [u8; 32] = padded_hex
                .try_into()
                .map_err(|_| Error::InvalidHash(x.to_string()))?;
            let hash = Hash(hash_bytes);
            let key = LedgerKey::ClaimableBalance(LedgerKeyClaimableBalance {
                balance_id: ClaimableBalanceIdTypeV0(hash),
            });
            ledger_keys.push(key);
        }
        Ok(())
    }
}