use std::{ffi::OsString, fmt::Display, fs, path::PathBuf};

use assert_cmd::{assert::Assert, Command};
use assert_fs::{prelude::PathChild, TempDir};
use sha2::{Digest, Sha256};
use soroban_env_host::xdr::{Error as XdrError, Hash, InstallContractCodeArgs, WriteXdr};

pub struct Wasm<'a>(pub &'a str);

impl Wasm<'_> {
    pub fn path(&self) -> PathBuf {
        let mut path = PathBuf::from(
            std::env::var("CARGO_MANIFEST_DIR")
                .map_or_else(|_| "", |_| "../..")
                .to_string(),
        )
        .join("target/wasm32-unknown-unknown/test-wasms")
        .join(self.0);
        path.set_extension("wasm");
        assert!(path.is_file(), "File not found: {}. run 'make build-test-wasms' to generate .wasm files before running this test", path.display());
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
    fn new_cmd(name: &str) -> Command {
        let mut this = Command::cargo_bin("soroban").expect("failed to find local soroban binary");
        this.arg(name);
        this
    }
}

/// Default
pub struct Sandbox {}

impl SorobanCommand for Sandbox {}

pub fn temp_ledger_file() -> OsString {
    TempDir::new()
        .unwrap()
        .child("ledger.json")
        .as_os_str()
        .into()
}

pub trait AssertExt {
    fn output_line(&self) -> String;
}

impl AssertExt for Assert {
    fn output_line(&self) -> String {
        String::from_utf8(self.get_output().stdout.clone())
            .expect("failed to make str")
            .trim()
            .to_owned()
    }
}
pub trait CommandExt {
    fn json_arg<A>(&mut self, j: A) -> &mut Self
    where
        A: Display;
}

impl CommandExt for Command {
    fn json_arg<A>(&mut self, j: A) -> &mut Self
    where
        A: Display,
    {
        self.arg(OsString::from(j.to_string()))
    }
}

// TODO add a `lib.rs` so that this can be imported
pub fn contract_hash(contract: &[u8]) -> Result<Hash, XdrError> {
    let args_xdr = InstallContractCodeArgs {
        code: contract.try_into()?,
    }
    .to_xdr()?;
    Ok(Hash(Sha256::digest(args_xdr).into()))
}

pub const HELLO_WORLD: &Wasm = &Wasm("test_hello_world");
pub const INVOKER_ACCOUNT_EXISTS: &Wasm = &Wasm("test_invoker_account_exists");
pub const CUSTOM_TYPES: &Wasm = &Wasm("test_custom_types");

#[allow(unused)]
pub fn temp_dir() -> TempDir {
    TempDir::new().unwrap()
}
