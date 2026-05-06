use super::args::Args;
use crate::xdr::{Hash, LedgerKey, LedgerKeyContractCode};
use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    /// Get WASM bytecode by hash
    #[arg(long)]
    pub wasm_hash: Vec<String>,

    #[command(flatten)]
    pub args: Args,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
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
        for hash in &self.wasm_hash {
            let hash = Hash(
                soroban_spec_tools::utils::contract_id_from_str(hash)
                    .map_err(|_| Error::InvalidHash(hash.clone()))?,
            );
            let key = LedgerKey::ContractCode(LedgerKeyContractCode { hash });
            ledger_keys.push(key);
        }

        Ok(())
    }
}
