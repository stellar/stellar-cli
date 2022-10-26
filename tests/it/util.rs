use std::{
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
};

use assert_cmd::{assert::Assert, cargo::CargoError, Command};
use assert_fs::{prelude::PathChild, TempDir};

pub fn wasm_file(name: &str) -> PathBuf {
    let path = Path::new(name);
    let wasm = if path
        .extension()
        .map_or(false, |ext| ext.eq_ignore_ascii_case("wasm"))
    {
        ""
    } else {
        ".wasm"
    };
    PathBuf::from("target/wasm32-unknown-unknown/test-wasms").join(format!("{name}{wasm}"))
}

pub struct Soroban {
    inner: Command,
}

impl Soroban {
    #[allow(unused)]
    pub fn arg<S>(&mut self, arg: S) -> &mut Self
    where
        S: AsRef<OsStr>,
    {
        self.inner.arg(arg);
        self
    }

    pub fn assert(&mut self) -> Assert {
        self.inner.assert()
    }

    pub fn new() -> Result<Self, CargoError> {
        Ok(Self {
            inner: Command::cargo_bin("soroban")?,
        })
    }

    pub fn new_standalone() -> Result<Self, CargoError> {
        let mut this = Self::new()?;
        this.set_standalone();
        Ok(this)
    }

    pub fn set_standalone(&mut self) -> &mut Self {
        self.inner
            .env("SOROBAN_RPC_URL", "http://localhost:8000/soroban/rpc")
            .env(
                "SOROBAN_SECRET_KEY",
                "SC5O7VZUXDJ6JBDSZ74DSERXL7W3Y5LTOAMRF7RQRL3TAGAPS7LUVG3L",
            )
            .env(
                "SOROBAN_NETWORK_PASSPHRASE",
                "Standalone Network ; February 2017",
            );
        self
    }

    pub fn invoke() -> Result<Self, CargoError> {
        let mut this = Self::new()?;
        this.inner.arg("invoke");
        Ok(this)
    }

    pub fn wasm<T>(&mut self, file: T) -> Result<&mut Self, anyhow::Error>
    where
        T: AsRef<OsStr>,
    {
        let path = PathBuf::from(file.as_ref());
        if path.is_file() {
            self.arg("--wasm");
            self.arg(path);
            Ok(self)
        } else {
            Err(anyhow::anyhow!("File not found: {}. run 'make test-wasms' to generate .wasm files before running this test", path.display()))
        }
    }

    pub fn ledger_file<T>(&mut self, file: T) -> &mut Self
    where
        T: AsRef<OsStr>,
    {
        self.arg("--ledger-file").arg(file);
        self
    }

    pub fn contract_id(&mut self, id: &str) -> &mut Self {
        self.arg("--id").arg(id);
        self
    }

    pub fn _fn(&mut self, name: &str) -> &mut Self {
        self.arg("--fn").arg(name);
        self
    }

    pub fn _arg<T>(&mut self, arg: T) -> &mut Self
    where
        T: AsRef<str>,
    {
        self.arg("--arg").arg(arg.as_ref());
        self
    }
}

pub fn temp_ledger_file() -> OsString {
    TempDir::new()
        .unwrap()
        .child("ledger.json")
        .as_os_str()
        .into()
}
