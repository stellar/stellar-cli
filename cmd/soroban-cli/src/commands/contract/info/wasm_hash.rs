use std::fmt::Debug;

use crate::commands::contract::info::shared::{self, fetch, Fetched};
use crate::commands::global;
use crate::print::Print;
use clap::{command, Parser};

#[derive(Parser, Debug, Clone)]
pub struct Cmd {
    #[command(flatten)]
    pub common: shared::Args,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Wasm(#[from] shared::Error),
    #[error("cannot get wasm hash from stellar asset contract")]
    StellarAssetContract,
    #[error("failed to calculate wasm hash from local file")]
    HashCalculation(#[from] crate::xdr::Error),
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let print = Print::new(global_args.quiet);
        let Fetched { contract, .. } = fetch(&self.common, &print).await?;

        let wasm_hash = match contract {
            shared::Contract::Wasm { wasm_bytes } => {
                // Calculate hash from wasm bytes
                hex::encode(crate::utils::contract_hash(&wasm_bytes)?)
            }
            shared::Contract::StellarAssetContract => {
                return Err(Error::StellarAssetContract);
            }
        };

        println!("{wasm_hash}");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wasm_hash_calculation() {
        // Test that we can calculate hash correctly for test wasm bytes
        let test_wasm = b"test wasm content";
        let hash = crate::utils::contract_hash(test_wasm).expect("hash calculation should work");
        let hex_hash = hex::encode(hash.0);
        
        // The hash should be a 64-character hex string (32 bytes)
        assert_eq!(hex_hash.len(), 64);
        
        // Test that same input produces same hash
        let hash2 = crate::utils::contract_hash(test_wasm).expect("hash calculation should work");
        let hex_hash2 = hex::encode(hash2.0);
        assert_eq!(hex_hash, hex_hash2);
    }
}