use std::ffi::OsString;

use soroban_test::{AssertExt, TestEnv};

use super::util::{extend_contract, new_account, AUTH, ED25519_ACCOUNT};

/// PATH with the directory `stellar` was built into, where `cargo build
/// --package stellar-signer-test-ed25519` puts the test signer plugin.
fn path_with_test_plugin() -> OsString {
    let stellar = assert_cmd::cargo::cargo_bin("stellar");
    let dir = stellar.parent().unwrap().to_path_buf();
    let paths = std::env::var_os("PATH").unwrap_or_default();
    std::env::join_paths(std::iter::once(dir).chain(std::env::split_paths(&paths))).unwrap()
}

/// Deploy a `test_ed25519_account` signed for by identity `signer`, and save
/// it as identity `name` using `plugin`.
async fn add_smart_account(sandbox: &TestEnv, name: &str, signer: &str, plugin: &str) -> String {
    let signer_pk = new_account(sandbox, signer);
    let signer_pk = stellar_strkey::ed25519::PublicKey::from_string(&signer_pk).unwrap();
    let account = sandbox
        .new_assert_cmd("contract")
        .arg("deploy")
        .arg("--source=test")
        .arg("--wasm")
        .arg(ED25519_ACCOUNT.path())
        .arg("--")
        .arg("--signer")
        .arg(hex::encode(signer_pk.0))
        .assert()
        .success()
        .stdout_as_str();
    extend_contract(sandbox, &account).await;
    sandbox
        .new_assert_cmd("keys")
        .args(["add", name, "--smart-account", &account])
        .args(["--signer-plugin", plugin, "--plugin-arg"])
        .arg(format!("key={signer}"))
        .assert()
        .success();
    account
}

async fn deploy_auth(sandbox: &TestEnv) -> String {
    let id = sandbox
        .new_assert_cmd("contract")
        .arg("deploy")
        .arg("--source=test")
        .arg("--wasm")
        .arg(AUTH.path())
        .arg("--")
        .arg("--addr=test")
        .assert()
        .success()
        .stdout_as_str();
    extend_contract(sandbox, &id).await;
    id
}

#[tokio::test]
async fn smart_account_signs_with_plugin() {
    let sandbox = &TestEnv::new();
    let account = add_smart_account(sandbox, "sa", "sa-signer", "test-ed25519").await;
    sandbox
        .new_assert_cmd("keys")
        .args(["address", "sa"])
        .assert()
        .success()
        .stdout(format!("{account}\n"));

    let id = deploy_auth(sandbox).await;
    sandbox
        .new_assert_cmd("contract")
        .env("PATH", path_with_test_plugin())
        .arg("invoke")
        .arg("--source=test")
        .arg("--id")
        .arg(&id)
        .arg("--")
        .arg("do-auth")
        .arg("--addr=sa")
        .arg("--val=hello")
        .assert()
        .success()
        .stdout("\"hello\"\n");
}

#[tokio::test]
async fn smart_account_plugin_not_installed() {
    let sandbox = &TestEnv::new();
    add_smart_account(sandbox, "sa", "sa-signer", "not-installed").await;

    let id = deploy_auth(sandbox).await;
    sandbox
        .new_assert_cmd("contract")
        .arg("invoke")
        .arg("--source=test")
        .arg("--id")
        .arg(&id)
        .arg("--")
        .arg("do-auth")
        .arg("--addr=sa")
        .arg("--val=hello")
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "signer plugin `stellar-signer-not-installed` not found on PATH",
        ));
}
