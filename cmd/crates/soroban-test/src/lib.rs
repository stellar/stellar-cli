#![allow(
    clippy::missing_errors_doc,
    clippy::must_use_candidate,
    clippy::missing_panics_doc
)]
use std::{env, ffi::OsString, fmt::Display, path::Path};

use assert_cmd::{assert::Assert, Command};
use assert_fs::{fixture::FixtureError, prelude::PathChild, TempDir};
use fs_extra::dir::CopyOptions;

pub use soroban_cli::commands::contract::invoke;
use soroban_cli::{commands::contract, CommandParser, Pwd};

mod wasm;
pub use wasm::Wasm;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Failed to create temporary directory")]
    TempDir(FixtureError),

    #[error(transparent)]
    FsError(#[from] fs_extra::error::Error),

    #[error(transparent)]
    Invoke(#[from] invoke::Error),
}

/// A `TestEnv` is a contained process for a specific test, with its own ENV and
/// its own `TempDir` where it will save test-specific configuration.
pub struct TestEnv {
    pub temp_dir: TempDir,
}

impl Default for TestEnv {
    fn default() -> Self {
        Self::new().unwrap()
    }
}

impl TestEnv {
    pub fn with_default<F: FnOnce(&TestEnv)>(f: F) {
        let test_env = TestEnv::default();
        env::set_var("SOROBAN_CONFIG_HOME", test_env.dir().path());
        f(&test_env);
        env::remove_var("SOROBAN_CONFIG_HOME");
    }
    pub fn new() -> Result<TestEnv, Error> {
        TempDir::new()
            .map_err(Error::TempDir)
            .map(|temp_dir| TestEnv { temp_dir })
    }
    pub fn new_cmd(&self, name: &str) -> Command {
        let mut this = Command::cargo_bin("soroban").unwrap_or_else(|_| Command::new("soroban"));
        this.arg(name);
        this.current_dir(&self.temp_dir);
        this
    }

    pub fn cmd<T: CommandParser<T>>(&self, args: &str) -> T {
        let args = format!("{args} --pwd={}", self.dir().display());
        T::parse(&args).unwrap()
    }

    pub fn invoke<I: AsRef<str>>(&self, command_str: &[I]) -> Result<String, Error> {
        let mut cmd = contract::invoke::Cmd::parse_arg_vec(
            &command_str.iter().map(AsRef::as_ref).collect::<Vec<_>>(),
        )
        .unwrap();
        cmd.set_pwd(self.dir());
        Ok(cmd.run_in_sandbox()?)
    }

    pub fn invoke_cmd(&self, mut cmd: invoke::Cmd) -> Result<String, Error> {
        cmd.config.locator.pwd = Some(self.dir().to_path_buf());
        Ok(cmd.run_in_sandbox()?)
    }

    pub fn dir(&self) -> &TempDir {
        &self.temp_dir
    }

    pub fn gen_test_identity(&self) {
        self.new_cmd("config")
            .arg("identity")
            .arg("generate")
            .arg("--seed")
            .arg("0000000000000000")
            .arg("test")
            .assert()
            .stdout("")
            .success();
    }

    pub fn test_address(&self, hd_path: usize) -> String {
        self.new_cmd("config")
            .args("identity address test --hd-path".split(' '))
            .arg(format!("{hd_path}"))
            .assert()
            .stdout_as_str()
    }

    pub fn fork(&self) -> Result<TestEnv, Error> {
        let this = TestEnv::new()?;
        self.save(&this.temp_dir)?;
        Ok(this)
    }

    /// Save the current state of the `TestEnv` to the given directory.
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

pub trait AssertExt {
    fn stdout_as_str(&self) -> String;
}

impl AssertExt for Assert {
    fn stdout_as_str(&self) -> String {
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
