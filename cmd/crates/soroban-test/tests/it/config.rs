use crate::util::{add_key, add_test_id, NoFund, SecretKind, DEFAULT_SEED_PHRASE};
use predicates::prelude::predicate;
use soroban_cli::commands::network;
use soroban_cli::config::network::passphrase::LOCAL as LOCAL_NETWORK_PASSPHRASE;
use soroban_test::{AssertExt, TestEnv};
use std::fs;

fn ls(sandbox: &TestEnv) -> Vec<String> {
    sandbox
        .new_assert_cmd("network")
        .arg("ls")
        .assert()
        .stdout_as_str()
        .split('\n')
        .filter(|s| !s.is_empty())
        .map(ToString::to_string)
        .collect::<Vec<_>>()
}

pub const NETWORKS: &str = r"local
futurenet
mainnet
testnet
";

#[test]
fn set_and_remove_network() {
    TestEnv::with_default(|sandbox| {
        add_network(sandbox, "custom");
        let dir = sandbox.config_dir().join("network");
        let mut read_dir = std::fs::read_dir(dir).unwrap();
        let file = read_dir.next().unwrap().unwrap();
        assert_eq!(file.file_name().to_str().unwrap(), "custom.toml");
        let res = ls(sandbox);
        assert_eq!(res[0], "custom");
        sandbox
            .new_assert_cmd("network")
            .arg("rm")
            .arg("custom")
            .assert()
            .success();

        sandbox
            .new_assert_cmd("network")
            .arg("ls")
            .assert()
            .stdout(NETWORKS);
    });
}

fn add_network(sandbox: &TestEnv, name: &str) {
    sandbox
        .new_assert_cmd("network")
        .arg("add")
        .args([
            "--rpc-url=https://127.0.0.1",
            "--network-passphrase",
            LOCAL_NETWORK_PASSPHRASE,
            name,
        ])
        .assert()
        .success()
        .stderr("")
        .stdout("");
}

#[test]
fn multiple_networks() {
    let sandbox = TestEnv::default();
    let ls = || -> Vec<String> { ls(&sandbox) };

    println!("{:#?}", ls());
    add_network(&sandbox, "custom");
    println!("{:#?}", ls());
    add_network(&sandbox, "local2");

    assert_eq!(
        ls().as_slice(),
        [
            "custom".to_owned(),
            "local2".to_owned(),
            "local".to_owned(),
            "futurenet".to_owned(),
            "mainnet".to_owned(),
            "testnet".to_owned()
        ]
    );

    sandbox.cmd::<network::rm::Cmd>("custom").run().unwrap();

    assert_eq!(
        ls().as_slice(),
        [
            "local2".to_owned(),
            "local".to_owned(),
            "futurenet".to_owned(),
            "mainnet".to_owned(),
            "testnet".to_owned()
        ]
    );

    #[cfg(feature = "version_lt_23")]
    {
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

        assert_eq!(
            ls().as_slice(),
            [
                "local2".to_owned(),
                "local3".to_owned(),
                "local".to_owned(),
                "futurenet".to_owned(),
                "mainnet".to_owned(),
                "testnet".to_owned()
            ]
        );
    }
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
        .no_fund()
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
        fs::read_to_string(sandbox.config_dir().join("identity/test_2.toml")).unwrap();
    assert_eq!(
        file_contents,
        format!("seed_phrase = \"{DEFAULT_SEED_PHRASE}\"\n")
    );
}

#[test]
fn generate_key_on_testnet() {
    if std::env::var("CI_TEST").is_err() {
        return;
    }
    let sandbox = TestEnv::default();
    sandbox
        .new_assert_cmd("keys")
        .arg("generate")
        .arg("--rpc-url=https://soroban-testnet.stellar.org:443")
        .arg("--network-passphrase=Test SDF Network ; September 2015")
        .arg("test_2")
        .assert()
        .stdout("")
        .stderr("")
        .success();

    sandbox
        .new_assert_cmd("keys")
        .arg("ls")
        .assert()
        .stdout(predicates::str::contains("test_2\n"));
    println!(
        "aa {}",
        sandbox
            .new_assert_cmd("keys")
            .arg("address")
            .arg("test_2")
            .assert()
            .success()
            .stdout_as_str()
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
        .arg("secret")
        .arg("bob")
        .assert()
        .success()
        .stdout("SDIY6AQQ75WMD4W46EYB7O6UYMHOCGQHLAQGQTKHDX4J2DYQCHVCQYFD\n");
}

#[cfg(feature = "version_lt_23")]
#[test]
fn config_dirs_precedence() {
    let sandbox = TestEnv::default();

    sandbox
        .new_assert_cmd("keys")
        .env(
            "SOROBAN_SECRET_KEY",
            "SC4ZPYELVR7S7EE7KZDZN3ETFTNQHHLTUL34NUAAWZG5OK2RGJ4V2U3Z",
        )
        .arg("add")
        .arg("alice")
        .assert()
        .success();

    fs::rename(
        sandbox.dir().join(".stellar"),
        sandbox.dir().join("_soroban"),
    )
    .unwrap();

    sandbox
        .new_assert_cmd("keys")
        .env(
            "SOROBAN_SECRET_KEY",
            "SAQMV6P3OWM2SKCK3OEWNXSRYWK5RNNUL5CPHQGIJF2WVT4EI2BZ63GG",
        )
        .arg("add")
        .arg("alice")
        .assert()
        .success();

    fs::rename(
        sandbox.dir().join("_soroban"),
        sandbox.dir().join(".soroban"),
    )
    .unwrap();

    sandbox
        .new_assert_cmd("keys")
        .arg("secret")
        .arg("alice")
        .arg("--verbose")
        .assert()
        .success()
        .stderr(predicate::str::contains(
            "WARN soroban_cli::utils: the .stellar and .soroban config directories exist at path",
        ))
        .stdout("SAQMV6P3OWM2SKCK3OEWNXSRYWK5RNNUL5CPHQGIJF2WVT4EI2BZ63GG\n");
}

#[test]
fn set_default_identity() {
    let sandbox = TestEnv::default();

    sandbox
        .new_assert_cmd("keys")
        .env(
            "SOROBAN_SECRET_KEY",
            "SC4ZPYELVR7S7EE7KZDZN3ETFTNQHHLTUL34NUAAWZG5OK2RGJ4V2U3Z",
        )
        .arg("add")
        .arg("alice")
        .assert()
        .success();

    sandbox
        .new_assert_cmd("keys")
        .arg("use")
        .arg("alice")
        .assert()
        .stderr(predicate::str::contains(
            "The default source account is set to `alice`",
        ))
        .success();

    sandbox
        .new_assert_cmd("env")
        .env_remove("SOROBAN_ACCOUNT")
        .assert()
        .stdout(predicate::str::contains("STELLAR_ACCOUNT=alice"))
        .success();
}

#[test]
fn set_default_network() {
    let sandbox = TestEnv::default();

    sandbox
        .new_assert_cmd("network")
        .arg("use")
        .arg("testnet")
        .assert()
        .stderr(predicate::str::contains(
            "The default network is set to `testnet`",
        ))
        .success();

    sandbox
        .new_assert_cmd("env")
        .assert()
        .stdout(predicate::str::contains("STELLAR_NETWORK=testnet"))
        .success();
}

#[test]
fn cannot_create_contract_with_test_name() {
    let sandbox = TestEnv::default();
    sandbox
        .new_assert_cmd("keys")
        .arg("generate")
        .no_fund()
        .arg("d")
        .assert()
        .success();
    sandbox
        .new_assert_cmd("contract")
        .arg("alias")
        .arg("add")
        .arg("d")
        .arg("--id=CA3D5KRYM6CB7OWQ6TWYRR3Z4T7GNZLKERYNZGGA5SOAOPIFY6YQGAXE")
        .assert()
        .stderr(predicate::str::contains("cannot overlap with key"))
        .failure();
}

#[test]
fn cannot_create_key_with_alias() {
    let sandbox = TestEnv::default();
    sandbox
        .new_assert_cmd("contract")
        .arg("alias")
        .arg("add")
        .arg("t")
        .arg("--id=CA3D5KRYM6CB7OWQ6TWYRR3Z4T7GNZLKERYNZGGA5SOAOPIFY6YQGAXE")
        .assert()
        .success();
    sandbox
        .new_assert_cmd("keys")
        .arg("generate")
        .no_fund()
        .arg("t")
        .assert()
        .stderr(predicate::str::contains(
            "cannot overlap with contract alias",
        ))
        .failure();
}
