use std::{ffi::OsString, path::PathBuf};

use assert_cmd::Command;
use assert_fs::{prelude::PathChild, TempDir};

pub fn test_wasm(name: &str) -> PathBuf {
    let mut path = PathBuf::from("../../target/wasm32-unknown-unknown/test-wasms").join(name);
    path.set_extension("wasm");
    assert!(path.is_file(), "File not found: {}. run 'make test-wasms' to generate .wasm files before running this test", path.display());
    path
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

pub fn temp_ledger_file() -> OsString {
    TempDir::new()
        .unwrap()
        .child("ledger.json")
        .as_os_str()
        .into()
}
