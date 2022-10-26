use std::{
    ffi::{OsStr, OsString},
    path::{Path,PathBuf},
};

use assert_cmd::{assert::Assert, cargo::CargoError, Command};
use assert_fs::{TempDir, prelude::PathChild};

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
            self.inner.arg("--wasm");
            self.inner.arg(path);
            Ok(self)
        } else {
            Err(anyhow::anyhow!("File not found: {}. run 'make test-wasms' to generate .wasm files before running this test", path.display()))
        }
    }

    pub fn ledger_file<T>(&mut self, file: T) -> &mut Self
    where
        T: AsRef<OsStr>,
    {
        self.inner.arg("--ledger-file");
        self.inner.arg(file);
        self
    }

    pub fn contract_id(&mut self, id: &str) -> &mut Self {
        self.inner.arg("--id");
        self.inner.arg(id);
        self
    }

    pub fn _fn(&mut self, name: &str) -> &mut Self {
        self.inner.arg("--fn");
        self.inner.arg(name);
        self
    }
}

pub fn temp_ledger_file() -> OsString {
    TempDir::new().unwrap().child("ledger.json").as_os_str().into()
}
