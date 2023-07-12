use std::{fmt::Display, fs, path::PathBuf};

use sha2::{Digest, Sha256};
use soroban_env_host::xdr;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Xdr(#[from] xdr::Error),
}

pub enum Wasm<'a> {
    Release(&'a str),
    Custom(&'a str, &'a str),
}

fn find_target_dir() -> Option<PathBuf> {
    let path = std::env::current_dir().unwrap();
    for parent in path.ancestors() {
        let path = parent.join("target");
        if path.is_dir() {
            return Some(path);
        }
    }
    None
}

impl Wasm<'_> {
    /// # Panics
    ///
    /// # if not found
    pub fn path(&self) -> PathBuf {
        let path = find_target_dir().unwrap().join("wasm32-unknown-unknown");
        let mut path = match self {
            Wasm::Release(name) => path.join("release").join(name),
            Wasm::Custom(profile, name) => path.join(profile).join(name),
        };
        path.set_extension("wasm");
        assert!(path.is_file(), "File not found: {}. run 'make build-test-wasms' to generate .wasm files before running this test", path.display());
        std::env::current_dir().unwrap().join(path)
    }

    /// # Panics
    ///
    /// # if not found
    pub fn bytes(&self) -> Vec<u8> {
        fs::read(self.path()).unwrap()
    }

    /// # Errors
    ///
    pub fn hash(&self) -> Result<xdr::Hash, Error> {
        Ok(xdr::Hash(Sha256::digest(self.bytes()).into()))
    }
}

impl Display for Wasm<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.path().display())
    }
}
