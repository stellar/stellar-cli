use std::{fs, path::PathBuf};

use sha2::{Digest, Sha256};
use soroban_env_host::xdr::{self, InstallContractCodeArgs, WriteXdr};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Xdr(#[from] xdr::Error),
}

/// Easily include a built Wasm artifact from your project's `target` folder
///
/// # Example
///
/// If your workspace includes a member with name "my-contract" (that is, with `name = "my-contract"` in its Cargo.toml), or if this is the name of your root project, then in your test you can use:
///
///     use soroban_test::{TestEnv, Wasm};
///     const WASM: &Wasm = &Wasm::Release("my_contract");
///
///     #[test]
///     fn invoke_and_read() {
///         TestEnv::with_default(|e| {
///             let output = e.new_cmd("contract")
///                 .arg("invoke")
///                 .arg("--wasm")
///                 .arg(&WASM.path())
///                 .ok();
///         })
///     }
pub enum Wasm<'a> {
    /// Takes a filename. Look inside `target/wasm32-unknown-unknown/release` for a Wasm file with the given name.
    Release(&'a str),

    /// Takes a `profile` and a filename. Look inside `target/wasm32-unknown-unknown/{profile}` for a given Wasm file with the given name.
    ///
    /// # Example
    ///
    ///     const WASM: &Wasm = &Wasm::Custom("debug", "my_contract");
    Custom(&'a str, &'a str),
}

fn find_target_dir() -> Option<PathBuf> {
    let path = std::env::current_dir().unwrap();
    for parent in path.ancestors().skip(1) {
        let path = parent.join("target");
        if path.is_dir() {
            return Some(path);
        }
    }
    None
}

impl Wasm<'_> {
    /// The file path to the specified Wasm file
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

    /// The bytes of the specified Wasm file
    pub fn bytes(&self) -> Vec<u8> {
        fs::read(self.path()).unwrap()
    }

    /// The derived hash of the specified Wasm file which will be used to refer to a contract "installed" in the Soroban blockchain (that is, to contract bytes that have been uploaded, and which zero or more contract instances may refer to for their behavior).
    pub fn hash(&self) -> Result<xdr::Hash, Error> {
        let args_xdr = InstallContractCodeArgs {
            code: self.bytes().try_into()?,
        }
        .to_xdr()?;
        Ok(xdr::Hash(Sha256::digest(args_xdr).into()))
    }
}
