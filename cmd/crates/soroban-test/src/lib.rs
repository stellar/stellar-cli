use std::{ffi::OsString, fmt::Display, path::Path};

use assert_cmd::{assert::Assert, Command};
use assert_fs::{fixture::FixtureError, prelude::PathChild, TempDir};
use fs_extra::dir::CopyOptions;
pub use wasm::Wasm;

mod wasm;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Failed to create temporary directory")]
    TempDir(FixtureError),

    #[error(transparent)]
    FsError(#[from] fs_extra::error::Error),
}

/// The primary interface to your tests. Creates an isolated process with its own temporary directory, storing all config and output there.
///
/// # Example:
///
///     use soroban_test::{TestEnv, Wasm};
///     const WASM: &Wasm = &Wasm::Release("my_contract");
///
///     #[test]
///     fn invoke_and_read() {
///         TestEnv::with_default(|e| {
///             e.new_cmd("contract")
///                 .arg("invoke")
///                 .arg("--wasm")
///                 .arg(&WASM.path())
///                 .args(["--fn", "some_fn"])
///                 .assert()
///                 .stderr("");
///         });
///     }
pub struct TestEnv {
    pub temp_dir: TempDir,
}

impl Default for TestEnv {
    fn default() -> Self {
        Self::new().unwrap()
    }
}

impl TestEnv {
    /// Initialize a TestEnv with default settings. Takes a closure to execute within this TestEnv.
    ///
    /// For now, this is the primary interface to create and use a TestEnv. In the future, TestEnv may provide an alternate, more customizable method of initialization.
    ///
    /// # Example
    ///
    /// In a test function:
    ///
    ///     TestEnv::with_default(|e| {
    ///         println!("{:#?}", e.new_cmd("version").ok());
    ///     });
    pub fn with_default<F: FnOnce(&TestEnv)>(f: F) {
        let test_env = TestEnv::default();
        f(&test_env)
    }

    /// Initialize a TestEnv with default settings and return a Result with this TestEnv or an error. You probably want `with_default` instead, which makes use of `new` internally.
    pub fn new() -> Result<TestEnv, Error> {
        TempDir::new()
            .map_err(Error::TempDir)
            .map(|temp_dir| TestEnv { temp_dir })
    }

    /// Start building a new `soroban` command, skipping the repetitive `soroban` starting word and setting the current directory to the one for this TestEnv.
    ///
    /// # Example
    ///
    ///     TestEnv::with_default(|e| {
    ///         println!("{:#?}", e.new_cmd("version").ok());
    ///     });
    ///
    /// Note that you don't need `e.new_cmd("soroban").arg("version")`.
    pub fn new_cmd(&self, name: &str) -> Command {
        let mut this = Command::cargo_bin("soroban").unwrap_or_else(|_| Command::new("soroban"));
        this.arg(name);
        this.current_dir(&self.temp_dir);
        this
    }

    /// Get the location of the temporary directory created by this TestEnv.
    pub fn dir(&self) -> &TempDir {
        &self.temp_dir
    }

    /// Generate new identity for testing. Names the identity `test`. Uses a hard-coded all-zero seed.
    pub fn gen_test_identity(&self) {
        self.new_cmd("config")
            .arg("identity")
            .arg("generate")
            .arg("--seed")
            .arg("0000000000000000")
            .arg("test")
            .assert()
            .success();
    }

    /// Return a public key of the identity named `test`, which needs to first be created with `gen_test_identity`. The `test` identity is stored as a seed, which can be used to generate multiple public keys. The specific key generated depends on the `hd_path` supplied.  Specifying the same `hd_path` will always generate the same key.
    ///
    /// The phrase "HD path" comes from the larger world of crypto wallets: https://www.ledger.com/academy/crypto/what-are-hierarchical-deterministic-hd-wallets
    pub fn test_address(&self, hd_path: usize) -> String {
        self.new_cmd("config")
            .args("identity address test --hd-path".split(' '))
            .arg(format!("{hd_path}"))
            .assert()
            .stdout_as_str()
    }

    /// Fork TestEnv, return a Result with the new TestEnv or an error. Might be useful to create multiple tests that use the same setup.
    pub fn fork(&self) -> Result<TestEnv, Error> {
        let this = TestEnv::new()?;
        self.save(&this.temp_dir)?;
        Ok(this)
    }

    /// Save the current state of the TestEnv to the given directory.
    pub fn save(&self, dst: &Path) -> Result<(), Error> {
        fs_extra::dir::copy(&self.temp_dir, dst, &CopyOptions::new())?;
        Ok(())
    }
}

pub fn temp_ledger_file() -> OsString {
    TempDir::new()
        .unwrap()
        .child("ledger.json")
        .as_os_str()
        .into()
}

/// Import this trait into your file to extend `assert_cmd` with extra utility functions.
///
/// `assert_cmd` is the CLI command builder powering soroban_test.
///
/// You don't need to do anything else with `AssertExt` other than import it; it magically extends the command builder with `stdout_as_str` and other helpers.
///
/// # Example:
///
///     use soroban_test::{AssertExt, TestEnv, Wasm};
///
///     const WASM: &Wasm = &Wasm::Release("my_contract");
///
///     #[test]
///     fn invoke() {
///         TestEnv::with_default(|e| {
///             let stdout = e
///                 .new_cmd("contract")
///                 .arg("install")
///                 .arg("--wasm")
///                 .arg(&WASM.path())
///                 .assert()
///                 .stdout_as_str();
///             println!("{stdout}");
///         }
///     }
///
/// Note that you need the `.assert()`, which is where `assert_cmd` executes the command and returns a struct to make assertions on.
pub trait AssertExt {
    /// If the command emits to STDOUT, this will return its output as a `&str`, with leading and trailing whitespace trimmed.
    fn stdout_as_str(&self) -> String;

    /// If the command emits to STDERR, this will return its output as a `&str`, with leading and trailing whitespace trimmed.
    fn stderr_as_str(&self) -> String;
}

impl AssertExt for Assert {
    fn stdout_as_str(&self) -> String {
        String::from_utf8(self.get_output().stdout.clone())
            .expect("failed to make str")
            .trim()
            .to_owned()
    }
    fn stderr_as_str(&self) -> String {
        String::from_utf8(self.get_output().stderr.clone())
            .expect("failed to make str")
            .trim()
            .to_owned()
    }
}

/// Import this trait into your file to extend `assert_cmd` with extra utility functions.
///
/// `assert_cmd` is the CLI command builder powering soroban_test.
///
/// You don't need to do anything else with `CommandExt` other than import it; it magically extends the command builder with `json_arg` and other helpers.
///
/// # Example:
///
///     use serde_json::json;
///     use soroban_test::{AssertExt, TestEnv};
///
///     #[test]
///     fn invoke() {
///         TestEnv::with_default(|e| {
///             e.new_cmd("contract")
///                 .arg("invoke")
///                 .args(["--id", "0"])
///                 .json_arg(json!({"ease": true, "pain": false}));
///         }
///     }
pub trait CommandExt {
    /// Pass json (constructed with the serde_json::json macro or similar) as a segment to your command.
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
