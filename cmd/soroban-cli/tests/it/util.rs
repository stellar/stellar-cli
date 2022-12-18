use std::{
    ffi::{OsStr, OsString},
    fs,
    path::{Path, PathBuf},
};

use assert_cmd::Command;
use assert_fs::{prelude::PathChild, TempDir};
use sha2::{Digest, Sha256};
use soroban_env_host::xdr::{Error as XdrError, Hash, InstallContractCodeArgs, WriteXdr};

// pub fn test_wasm(name: &str) -> PathBuf {}

pub struct Wasm<'a>(pub &'a str);

impl Wasm<'_> {
    pub fn path(&self) -> PathBuf {
        let mut path = PathBuf::from("../../target/wasm32-unknown-unknown/test-wasms").join(self.0);
        path.set_extension("wasm");
        assert!(path.is_file(), "File not found: {}. run 'make test-wasms' to generate .wasm files before running this test", path.display());
        path
    }

    pub fn bytes(&self) -> Vec<u8> {
        fs::read(self.path()).unwrap()
    }

    pub fn hash(&self) -> Hash {
        contract_hash(&self.bytes()).unwrap()
    }
}

/// Create a command with the correct env variables
pub trait SorobanCommand {
    /// Default is with none
    fn new_cmd() -> Command {
        Command::cargo_bin("soroban").expect("failed to find local soroban binary")
    }
}

/// Default
pub struct Sandbox {}

impl SorobanCommand for Sandbox {}

/// Standalone Network
pub struct Standalone {}

impl SorobanCommand for Standalone {
    fn new_cmd() -> Command {
        let mut this = Sandbox::new_cmd();
        this.env("SOROBAN_RPC_URL", "http://localhost:8000/soroban/rpc")
            .env(
                "SOROBAN_SECRET_KEY",
                "SC5O7VZUXDJ6JBDSZ74DSERXL7W3Y5LTOAMRF7RQRL3TAGAPS7LUVG3L",
            )
            .env(
                "SOROBAN_NETWORK_PASSPHRASE",
                "Standalone Network ; February 2017",
            );
        this
    }
}

pub fn temp_ledger_file() -> OsString {
    TempDir::new()
        .unwrap()
        .child("ledger.json")
        .as_os_str()
        .into()
}

// TODO add a `lib.rs` so that this can be imported
pub fn contract_hash(contract: &[u8]) -> Result<Hash, XdrError> {
    let args_xdr = InstallContractCodeArgs {
        code: contract.try_into()?,
    }
    .to_xdr()?;
    Ok(Hash(Sha256::digest(args_xdr).into()))
}

pub const hello_world: &Wasm = &Wasm("test_hello_world");
pub const invoker_account_exists: &Wasm = &Wasm("test_invoker_account_exists");
pub const custom_types: &Wasm = &Wasm("test_custom_types");
