use std::{fmt::Display, path::Path};

use assert_cmd::Command;
use soroban_cli::commands::config::{locator::KeyType, secret::Secret};
use soroban_test::{TestEnv, Wasm};

pub const HELLO_WORLD: &Wasm = &Wasm::Custom("test-wasms", "test_hello_world");
pub const CUSTOM_TYPES: &Wasm = &Wasm::Custom("test-wasms", "test_custom_types");

#[derive(Clone)]
pub enum SecretKind {
    Seed,
    Key,
}

#[allow(clippy::needless_pass_by_value)]
pub fn add_identity(dir: &Path, name: &str, kind: SecretKind, data: &str) {
    let secret = match kind {
        SecretKind::Seed => Secret::SeedPhrase {
            seed_phrase: data.to_string(),
        },
        SecretKind::Key => Secret::SecretKey {
            secret_key: data.to_string(),
        },
    };

    KeyType::Identity
        .write(name, &secret, &dir.join(".soroban"))
        .unwrap();
}

pub fn add_test_id(dir: &Path) -> String {
    let name = "test_id";
    add_identity(
        dir,
        name,
        SecretKind::Key,
        "SBGWSG6BTNCKCOB3DIFBGCVMUPQFYPA2G4O34RMTB343OYPXU5DJDVMN",
    );
    name.to_owned()
}

pub fn add_test_seed(dir: &Path) -> String {
    let name = "test_seed";
    add_identity(
        dir,
        name,
        SecretKind::Seed,
        "coral light army gather adapt blossom school alcohol coral light army giggle",
    );
    name.to_owned()
}

pub fn invoke(sandbox: &TestEnv, func: &str) -> Command {
    let mut s = sandbox.new_assert_cmd("contract");
    s.arg("invoke")
        .arg("--id=1")
        .arg("--wasm")
        .arg(CUSTOM_TYPES.path())
        .arg("--")
        .arg(func);
    s
}

pub fn invoke_with_roundtrip<D>(func: &str, data: D)
where
    D: Display,
{
    TestEnv::with_default(|e| {
        let data = data.to_string();
        println!("{data}");
        let res = e
            .invoke(&[
                "--id=1",
                "--wasm",
                &CUSTOM_TYPES.to_string(),
                "--",
                func,
                &format!("--{func}"),
                &data,
            ])
            .unwrap();
        assert_eq!(res, data);
    });
}

pub const DEFAULT_SEED_PHRASE: &str =
    "coral light army gather adapt blossom school alcohol coral light army giggle";

pub const DEFAULT_PUB_KEY: &str = "GDIY6AQQ75WMD4W46EYB7O6UYMHOCGQHLAQGQTKHDX4J2DYQCHVCR4W4";
pub const DEFAULT_SECRET_KEY: &str = "SC36BWNUOCZAO7DMEJNNKFV6BOTPJP7IG5PSHLUOLT6DZFRU3D3XGIXW";

pub const DEFAULT_PUB_KEY_1: &str = "GCKZUJVUNEFGD4HLFBUNVYM2QY2P5WQQZMGRA3DDL4HYVT5MW5KG3ODV";
