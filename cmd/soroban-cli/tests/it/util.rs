use std::{fmt::Display, fs};

use assert_cmd::Command;
use assert_fs::TempDir;
use soroban_test::{CommandExt, Nebula, Wasm};

pub const HELLO_WORLD: &Wasm = &Wasm::Custom("test-wasms", "test_hello_world");
pub const CUSTOM_TYPES: &Wasm = &Wasm::Custom("test-wasms", "test_custom_types");

#[derive(Clone)]
pub enum SecretKind {
    Seed,
    Key,
}

#[allow(clippy::needless_pass_by_value)]
pub fn add_identity(dir: &TempDir, name: &str, kind: SecretKind, data: &str) {
    let identity_dir = dir.join(".soroban").join("identities");
    fs::create_dir_all(&identity_dir).unwrap();
    let kind_str = match kind {
        SecretKind::Seed => "seed",
        SecretKind::Key => "secret_key",
    };
    fs::write(
        identity_dir.join(format!("{name}.toml")),
        format!("{kind_str} = {data}\n"),
    )
    .unwrap();
}

pub fn add_test_id(dir: &TempDir) -> String {
    let name = "test_id";
    add_identity(
        dir,
        name,
        SecretKind::Key,
        "SBGWSG6BTNCKCOB3DIFBGCVMUPQFYPA2G4O34RMTB343OYPXU5DJDVMN",
    );
    name.to_owned()
}

pub fn add_test_seed(dir: &TempDir) -> String {
    let name = "test_seed";
    add_identity(
        dir,
        name,
        SecretKind::Seed,
        "one two three four five six seven eight nine ten eleven twelve",
    );
    name.to_owned()
}

pub fn invoke(sandbox: &Nebula, func: &str) -> Command {
    let mut s = sandbox.new_cmd("contract");
    s.arg("invoke")
        .arg("--id=1")
        .arg("--wasm")
        .arg(CUSTOM_TYPES.path())
        .arg("--fn")
        .arg(func)
        .arg("--");
    s
}

pub fn invoke_with_roundtrip<D>(func: &str, data: D)
where
    D: Display,
{
    invoke(&Nebula::default(), func)
        .arg(&format!("--{func}"))
        .json_arg(&data)
        .assert()
        .success()
        .stdout(format!("{data}\n"));
}

pub const DEFAULT_SEED_PHRASE: &str =
    "coral light army gather adapt blossom school alcohol coral light army giggle";
