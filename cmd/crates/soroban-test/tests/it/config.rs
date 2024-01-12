use assert_fs::TempDir;
use soroban_test::TestEnv;
use std::{fs, path::Path};

use crate::util::{add_key, add_test_id, SecretKind, DEFAULT_SEED_PHRASE};
use soroban_cli::commands::network;

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
            .new_assert_cmd("network")
            .arg("ls")
            .assert()
            .stdout("\n");
    });
}

fn add_network(sandbox: &TestEnv, name: &str) {
    sandbox
        .new_assert_cmd("network")
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
        .new_assert_cmd("network")
        .env("XDG_CONFIG_HOME", dir.to_str().unwrap())
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
        .new_assert_cmd("network")
        .env("XDG_CONFIG_HOME", dir.to_str().unwrap())
        .arg("ls")
        .arg("--global")
        .assert()
        .stdout("global\n");

    sandbox
        .new_assert_cmd("network")
        .env("XDG_CONFIG_HOME", dir.to_str().unwrap())
        .arg("rm")
        .arg("--global")
        .arg("global")
        .assert()
        .stdout("");

    sandbox
        .new_assert_cmd("network")
        .env("XDG_CONFIG_HOME", dir.to_str().unwrap())
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
fn read_key() {
    let sandbox = TestEnv::default();
    let dir = sandbox.dir().as_ref();
    add_test_id(dir);
    let ident_dir = dir.join(".soroban/identity");
    assert!(ident_dir.exists());
    sandbox
        .new_assert_cmd("keys")
        .arg("ls")
        .assert()
        .stdout(predicates::str::contains("test_id\n"));
}

#[test]
fn generate_key() {
    let sandbox = TestEnv::default();
    sandbox
        .new_assert_cmd("keys")
        .arg("generate")
        .arg("--network=futurenet")
        .arg("--no-fund")
        .arg("--seed")
        .arg("0000000000000000")
        .arg("test_2")
        .assert()
        .stdout("")
        .success();

    sandbox
        .new_assert_cmd("keys")
        .arg("ls")
        .assert()
        .stdout(predicates::str::contains("test_2\n"));
    let file_contents =
        fs::read_to_string(sandbox.dir().join(".soroban/identity/test_2.toml")).unwrap();
    assert_eq!(
        file_contents,
        format!("seed_phrase = \"{DEFAULT_SEED_PHRASE}\"\n")
    );
}

#[test]
fn seed_phrase() {
    let sandbox = TestEnv::default();
    let dir = sandbox.dir();
    add_key(
        dir,
        "test_seed",
        SecretKind::Seed,
        "one two three four five six seven eight nine ten eleven twelve",
    );

    sandbox
        .new_assert_cmd("keys")
        .current_dir(dir)
        .arg("ls")
        .assert()
        .stdout(predicates::str::contains("test_seed\n"));
}

#[test]
fn use_env() {
    let sandbox = TestEnv::default();

    sandbox
        .new_assert_cmd("keys")
        .env(
            "SOROBAN_SECRET_KEY",
            "SDIY6AQQ75WMD4W46EYB7O6UYMHOCGQHLAQGQTKHDX4J2DYQCHVCQYFD",
        )
        .arg("add")
        .arg("bob")
        .assert()
        .stdout("")
        .success();

    sandbox
        .new_assert_cmd("keys")
        .arg("show")
        .arg("bob")
        .assert()
        .success()
        .stdout("SDIY6AQQ75WMD4W46EYB7O6UYMHOCGQHLAQGQTKHDX4J2DYQCHVCQYFD\n");
}
