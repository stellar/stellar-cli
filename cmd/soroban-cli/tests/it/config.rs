use assert_cmd::Command;

use assert_fs::TempDir;
use soroban_test::{temp_ledger_file, TestEnv};
use std::{fs, path::Path};

use crate::util::{add_identity, add_test_id, SecretKind, DEFAULT_SEED_PHRASE, HELLO_WORLD};

#[test]
fn set_and_remove_network() {
    let sandbox = TestEnv::default();
    sandbox
        .new_cmd("config")
        .arg("network")
        .arg("add")
        .arg("--rpc-url")
        .arg("https://127.0.0.1")
        .arg("local")
        .arg("--network-passphrase")
        .arg("Local Sandbox Stellar Network ; September 2022")
        .assert()
        .success();
    let dir = &sandbox.temp_dir;
    let file = std::fs::read_dir(dir.join(".soroban/networks"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap();
    assert_eq!(file.file_name().to_str().unwrap(), "local.toml");

    sandbox
        .new_cmd("config")
        .arg("network")
        .arg("ls")
        .assert()
        .stdout("local\n");

    sandbox
        .new_cmd("config")
        .arg("network")
        .arg("rm")
        .arg("local")
        .assert()
        .stdout("");
    sandbox
        .new_cmd("config")
        .arg("network")
        .arg("ls")
        .assert()
        .stdout("\n");
}

fn add_network(sandbox: &TestEnv, name: &str) -> Command {
    let mut cmd = sandbox.new_cmd("config");
    cmd.arg("network")
        .arg("add")
        .arg("--rpc-url")
        .arg("https://127.0.0.1")
        .arg("--network-passphrase")
        .arg("Local Sandbox Stellar Network ; September 2022")
        .arg(name);
    cmd
}

fn add_network_global(sandbox: &TestEnv, dir: &Path, name: &str) {
    sandbox
        .new_cmd("config")
        .env("XDG_CONFIG_HOME", dir.to_str().unwrap())
        .arg("network")
        .arg("add")
        .arg("--global")
        .arg("--rpc-url")
        .arg("https://127.0.0.1")
        .arg("--network-passphrase")
        .arg("Local Sandbox Stellar Network ; September 2022")
        .arg(name)
        .assert()
        .success();
}

#[test]
fn set_and_remove_global_network() {
    let sandbox = TestEnv::default();
    let dir = TempDir::new().unwrap();

    add_network_global(&sandbox, &dir, "global");

    sandbox
        .new_cmd("config")
        .env("XDG_CONFIG_HOME", dir.to_str().unwrap())
        .arg("network")
        .arg("ls")
        .arg("--global")
        .assert()
        .stdout("global\n");

    sandbox
        .new_cmd("config")
        .env("XDG_CONFIG_HOME", dir.to_str().unwrap())
        .arg("network")
        .arg("rm")
        .arg("--global")
        .arg("global")
        .assert()
        .stdout("");

    sandbox
        .new_cmd("config")
        .env("XDG_CONFIG_HOME", dir.to_str().unwrap())
        .arg("network")
        .arg("ls")
        .assert()
        .stdout("\n");
}

#[test]
fn mulitple_networks() {
    let sandbox = TestEnv::default();

    add_network(&sandbox, "local").assert().success();
    add_network(&sandbox, "local2").assert().success();

    sandbox
        .new_cmd("config")
        .arg("network")
        .arg("ls")
        .assert()
        .stdout("local\nlocal2\n");

    sandbox
        .new_cmd("config")
        .arg("network")
        .arg("rm")
        .arg("local")
        .assert();
    sandbox
        .new_cmd("config")
        .arg("network")
        .arg("ls")
        .assert()
        .stdout("local2\n");

    let sub_dir = sandbox.dir().join("sub_directory");
    fs::create_dir(&sub_dir).unwrap();
    add_network(&sandbox, "local3\n")
        .current_dir(sub_dir)
        .assert()
        .success();

    sandbox
        .new_cmd("config")
        .arg("network")
        .arg("ls")
        .assert()
        .stdout("local2\nlocal3\n");
}

#[test]
fn read_identity() {
    let sandbox = TestEnv::default();
    add_test_id(&sandbox.temp_dir);
    sandbox
        .new_cmd("config")
        .arg("identity")
        .arg("ls")
        .assert()
        .stdout("test_id\n");
}

#[test]
fn generate_identity() {
    let sandbox = TestEnv::default();
    sandbox.gen_test_identity();

    sandbox
        .new_cmd("config")
        .arg("identity")
        .arg("ls")
        .assert()
        .stdout("test\n");
    let file_contents =
        fs::read_to_string(sandbox.dir().join(".soroban/identities/test.toml")).unwrap();
    assert_eq!(
        file_contents,
        format!("seed_phrase = \"{DEFAULT_SEED_PHRASE}\"\n")
    );
}

#[test]
fn seed_phrase() {
    let sandbox = TestEnv::default();
    let dir = sandbox.dir();
    add_identity(
        dir,
        "test_seed",
        SecretKind::Seed,
        "one two three four five six seven eight nine ten eleven twelve",
    );

    sandbox
        .new_cmd("config")
        .current_dir(dir)
        .arg("identity")
        .arg("ls")
        .assert()
        .stdout("test_seed\n");
}

#[test]
fn use_different_ledger_file() {
    let sandbox = TestEnv::default();
    sandbox
        .new_cmd("contract")
        .arg("invoke")
        .arg("--id=1")
        .arg("--wasm")
        .arg(HELLO_WORLD.path())
        .arg("--ledger-file")
        .arg(temp_ledger_file())
        .arg("--")
        .arg("hello")
        .arg("--world=world")
        .assert()
        .stdout("[\"Hello\",\"world\"]\n")
        .success();
    assert!(fs::read(sandbox.dir().join(".soroban/ledger.json")).is_err());
}

#[test]
fn read_address() {
    let sandbox = TestEnv::default();
    sandbox.gen_test_identity();
    for hd_path in 0..2 {
        test_hd_path(&sandbox, hd_path);
    }
}

#[test]
fn use_env() {
    let sandbox = TestEnv::default();

    sandbox
        .new_cmd("config")
        .env(
            "SOROBAN_SECRET_KEY",
            "SDIY6AQQ75WMD4W46EYB7O6UYMHOCGQHLAQGQTKHDX4J2DYQCHVCQYFD",
        )
        .arg("identity")
        .arg("add")
        .arg("bob")
        .assert()
        .stdout("")
        .success();

    sandbox
        .new_cmd("config")
        .arg("identity")
        .arg("show")
        .arg("bob")
        .assert()
        .success()
        .stdout("SDIY6AQQ75WMD4W46EYB7O6UYMHOCGQHLAQGQTKHDX4J2DYQCHVCQYFD\n");
}


fn test_hd_path(sandbox: &TestEnv, hd_path: usize) {
    let seed_phrase = sep5::SeedPhrase::from_seed_phrase(DEFAULT_SEED_PHRASE).unwrap();
    let key_pair = seed_phrase.from_path_index(hd_path, None).unwrap();
    let pub_key = key_pair.public().to_string();
    let test_address = sandbox.test_address(hd_path);
    assert_eq!(pub_key, test_address);
}
