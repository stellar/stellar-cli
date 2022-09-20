use std::fmt::Debug;

use clap::Parser;
use hex::FromHexError;

use crate::utils;

use super::Remote;

#[derive(Parser, Debug)]
pub struct Cmd {
    /// Contract ID to invoke
    #[clap(long = "id")]
    contract_id: String,
    /// Function name to execute
    #[clap(long = "fn")]
    function: String,
    /// Argument to pass to the function
    #[clap(long = "arg", value_name = "arg", multiple = true)]
    args: Vec<String>,
    /// Argument to pass to the function (base64-encoded xdr)
    #[clap(long = "arg-xdr", value_name = "arg-xdr", multiple = true)]
    args_xdr: Vec<String>,
    /// Output the cost execution to stderr
    #[clap(long = "cost")]
    cost: bool,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("cannot parse contract ID {contract_id}: {error}")]
    CannotParseContractId {
        contract_id: String,
        error: FromHexError,
    },
}

// TODO: Copy / share logic from sandbox::invoke for parsing args.

impl Cmd {
    pub fn run(&self, _remote: &Remote, _matches: &clap::ArgMatches) -> Result<(), Error> {
        let _contract_id: [u8; 32] =
            utils::contract_id_from_str(&self.contract_id).map_err(|e| {
                Error::CannotParseContractId {
                    contract_id: self.contract_id.clone(),
                    error: e,
                }
            })?;

        // TODO: Get contract WASM file from remote for extracting inputs from
        // contract spec.

        // TODO: Parse args based on contract spec, similar to sandbox::invoke.

        // TODO: Invoke contract on remote RPC/Horizon.

        // TODO: Print result.

        // TODO: Print events.

        Ok(())
    }
}
