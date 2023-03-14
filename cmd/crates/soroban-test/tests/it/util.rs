use std::{fmt::Display, fs};

use assert_cmd::Command;
use assert_fs::TempDir;
use soroban_test::{TestEnv, Wasm};

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
        SecretKind::Seed => "seed_phrase",
        SecretKind::Key => "secret_key",
    };
    let contents = format!("{kind_str} = \"{data}\"\n");
    fs::write(identity_dir.join(format!("{name}.toml")), contents).unwrap();
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
        "coral light army gather adapt blossom school alcohol coral light army giggle",
    );
    name.to_owned()
}

pub fn invoke(sandbox: &TestEnv, func: &str) -> Command {
    let mut s = sandbox.new_cmd("contract");
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
        let res = e
            .invoke(format!(
                "invoke --id=1 --wasm={CUSTOM_TYPES} -- {func} --{func} {data}",
            ))
            .unwrap();
        assert_eq!(res, data.to_string());
    });
}

pub const DEFAULT_SEED_PHRASE: &str =
    "coral light army gather adapt blossom school alcohol coral light army giggle";

pub const DEFAULT_PUB_KEY: &str = "GDIY6AQQ75WMD4W46EYB7O6UYMHOCGQHLAQGQTKHDX4J2DYQCHVCR4W4";
pub const DEFAULT_SECRET_KEY: &str = "SC36BWNUOCZAO7DMEJNNKFV6BOTPJP7IG5PSHLUOLT6DZFRU3D3XGIXW";

pub const DEFAULT_PUB_KEY_1: &str = "GCKZUJVUNEFGD4HLFBUNVYM2QY2P5WQQZMGRA3DDL4HYVT5MW5KG3ODV";
