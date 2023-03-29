use assert_fs::TempDir;
use soroban_test::{temp_ledger_file, TestEnv};
use std::{fs, path::Path};

use crate::util::{add_identity, add_test_id, SecretKind, DEFAULT_SEED_PHRASE, HELLO_WORLD};
use soroban_cli::commands::config::network;

const NETWORK_PASSPHRASE: &str = "Local Sandbox Stellar Network ; September 2022";

#[test]
fn set_and_remove_network() {
    TestEnv::with_default(|sandbox| {
        add_network(sandbox, "local");
        let dir = sandbox.dir().join(".soroban").join("network");
        let read_dir = std::fs::read_dir(dir);
        println!("{read_dir:#?}");
        let file = read_dir.unwrap().next().unwrap().unwrap();
        assert_eq!(file.file_name().to_str().unwrap(), "local.toml");

        let res = sandbox.cmd::<network::ls::Cmd>("");
        let res = res.ls().unwrap();
        assert_eq!(res.len(), 1);
        assert_eq!(&res[0], "local");

        sandbox.cmd::<network::rm::Cmd>("local").run().unwrap();

        // sandbox
        //     .new_assert_cmd("config")
        //     .arg("network")
        //     .arg("rm")
        //     .arg("local")
        //     .assert()
        //     .stdout("");
        sandbox
            .new_assert_cmd("config")
            .arg("network")
            .arg("ls")
            .assert()
            .stdout("\n");
    });
}

fn add_network(sandbox: &TestEnv, name: &str) {
    sandbox
        .new_assert_cmd("config")
        .arg("network")
        .arg("add")
        .args([
            "--rpc-url=https://127.0.0.1",
            "--network-passphrase",
            NETWORK_PASSPHRASE,
            name,
        ])
        .assert()
        .success()
        .stderr("")
        .stdout("");
}

fn add_network_global(sandbox: &TestEnv, dir: &Path, name: &str) {
    sandbox
        .new_assert_cmd("config")
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
        .new_assert_cmd("config")
        .env("XDG_CONFIG_HOME", dir.to_str().unwrap())
        .arg("network")
        .arg("ls")
        .arg("--global")
        .assert()
        .stdout("global\n");

    sandbox
        .new_assert_cmd("config")
        .env("XDG_CONFIG_HOME", dir.to_str().unwrap())
        .arg("network")
        .arg("rm")
        .arg("--global")
        .arg("global")
        .assert()
        .stdout("");

    sandbox
        .new_assert_cmd("config")
        .env("XDG_CONFIG_HOME", dir.to_str().unwrap())
        .arg("network")
        .arg("ls")
        .assert()
        .stdout("\n");
}

#[test]
fn multiple_networks() {
    let sandbox = TestEnv::default();
    let ls = || -> Vec<String> { sandbox.cmd::<network::ls::Cmd>("").ls().unwrap() };

    add_network(&sandbox, "local");
    println!("{:#?}", ls());
    add_network(&sandbox, "local2");

    assert_eq!(ls().as_slice(), ["local".to_owned(), "local2".to_owned()]);

    sandbox.cmd::<network::rm::Cmd>("local").run().unwrap();

    assert_eq!(ls().as_slice(), ["local2".to_owned()]);

    let sub_dir = sandbox.dir().join("sub_directory");
    fs::create_dir(&sub_dir).unwrap();

    TestEnv::cmd_arr_with_pwd::<network::add::Cmd>(
        &[
            "--rpc-url",
            "https://127.0.0.1",
            "--network-passphrase",
            "Local Sandbox Stellar Network ; September 2022",
            "local3",
        ],
        &sub_dir,
    )
    .run()
    .unwrap();

    assert_eq!(ls().as_slice(), ["local2".to_owned(), "local3".to_owned()]);
}

#[test]
fn read_identity() {
    let sandbox = TestEnv::default();
    let dir = sandbox.dir().as_ref();
    add_test_id(dir);
    let ident_dir = dir.join(".soroban/identity");
    assert!(ident_dir.exists());
    sandbox
        .new_assert_cmd("config")
        .arg("identity")
        .arg("ls")
        .assert()
        .stdout("test_id\n");
}

#[test]
fn generate_identity() {
    let sandbox = TestEnv::default();
    sandbox
        .new_assert_cmd("config")
        .arg("identity")
        .arg("generate")
        .arg("--seed")
        .arg("0000000000000000")
        .arg("test")
        .assert()
        .stdout("")
        .success();

    sandbox
        .new_assert_cmd("config")
        .arg("identity")
        .arg("ls")
        .assert()
        .stdout("test\n");
    let file_contents =
        fs::read_to_string(sandbox.dir().join(".soroban/identity/test.toml")).unwrap();
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
        .new_assert_cmd("config")
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
        .new_assert_cmd("contract")
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
    for hd_path in 0..2 {
        test_hd_path(&sandbox, hd_path);
    }
}

#[test]
fn use_env() {
    let sandbox = TestEnv::default();

    sandbox
        .new_assert_cmd("config")
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
        .new_assert_cmd("config")
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
