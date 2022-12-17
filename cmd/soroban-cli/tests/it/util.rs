use std::{ffi::OsString, fmt::Display, path::PathBuf};

use assert_cmd::{assert::Assert, Command};
use assert_fs::{prelude::PathChild, TempDir};

pub fn test_wasm(name: &str) -> PathBuf {
    let mut path = PathBuf::from(
        std::env::var("CARGO_MANIFEST_DIR")
            .map_or_else(|_| "", |_| "../..")
            .to_string(),
    )
    .join("target/wasm32-unknown-unknown/test-wasms")
    .join(name);
    path.set_extension("wasm");
    assert!(path.is_file(), "File not found: {}. run 'make build-test-wasms' to generate .wasm files before running this test", path.display());
    path
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

/// Standalone Network
pub struct Standalone {}

impl SorobanCommand for Standalone {
    fn new_cmd(name: &str) -> Command {
        let mut this = Sandbox::new_cmd(name);
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

pub fn output_line(a: &Assert) -> String {
    a.output_line()
}
