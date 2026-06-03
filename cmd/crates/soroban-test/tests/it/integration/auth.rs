use assert_cmd::Command;
use soroban_test::{AssertExt, TestEnv};

use super::util::{extend_contract, new_account, AUTH};

fn constructor_cmd(sandbox: &TestEnv, addr: &str) -> Command {
    let mut cmd = sandbox.new_assert_cmd("contract");
    cmd.arg("deploy")
        .arg("--source=test")
        .arg("--wasm")
        .arg(AUTH.path());
    cmd.arg("--").arg("--addr").arg(addr);
    cmd
}

/// Helper to deploy two instances of the auth contract and extend them.
/// Returns (contract_id_1, contract_id_2).
async fn deploy_auth_contracts(sandbox: &TestEnv) -> (String, String) {
    let id1 = constructor_cmd(sandbox, "test")
        .assert()
        .success()
        .stdout_as_str();
    extend_contract(sandbox, &id1).await;

    let id2 = constructor_cmd(sandbox, "test")
        .assert()
        .success()
        .stdout_as_str();
    extend_contract(sandbox, &id2).await;

    (id1, id2)
}

#[tokio::test]
async fn standard_auth_with_separate_signer() {
    let sandbox = &TestEnv::new();
    new_account(sandbox, "signer");

    let (id, _) = deploy_auth_contracts(sandbox).await;

    sandbox
        .new_assert_cmd("contract")
        .arg("invoke")
        .arg("--source=test")
        .arg("--id")
        .arg(&id)
        .arg("--")
        .arg("do-auth")
        .arg("--addr=signer")
        .arg("--val=hello")
        .assert()
        .success()
        .stdout("\"hello\"\n");
}

#[tokio::test]
async fn root_auth_with_authorized_subcall() {
    let sandbox = &TestEnv::new();
    new_account(sandbox, "signer");

    let (id1, id2) = deploy_auth_contracts(sandbox).await;

    sandbox
        .new_assert_cmd("contract")
        .arg("invoke")
        .arg("--source=test")
        .arg("--id")
        .arg(&id1)
        .arg("--")
        .arg("auth-sub-auth")
        .arg("--addr=signer")
        .arg("--val=hello")
        .arg(&format!("--subcall={id2}"))
        .assert()
        .success()
        .stdout("\"hello\"\n");
}

#[tokio::test]
async fn non_root_auth_with_authorized_subcall() {
    let sandbox = &TestEnv::new();
    new_account(sandbox, "signer");

    let (id1, id2) = deploy_auth_contracts(sandbox).await;

    // with non-source signer - expect failure
    sandbox
        .new_assert_cmd("contract")
        .arg("invoke")
        .arg("--source=test")
        .arg("--id")
        .arg(&id1)
        .arg("--")
        .arg("no-auth-sub-auth")
        .arg("--addr=signer")
        .arg("--val=hello")
        .arg(&format!("--subcall={id2}"))
        .assert()
        .failure()
        .stderr(predicates::str::contains("Auth, InvalidAction"));

    // with source signer - expect failure due to default root auth mode
    sandbox
        .new_assert_cmd("contract")
        .arg("invoke")
        .arg("--source=test")
        .arg("--id")
        .arg(&id1)
        .arg("--")
        .arg("no-auth-sub-auth")
        .arg("--addr=test")
        .arg("--val=hello")
        .arg(&format!("--subcall={id2}"))
        .assert()
        .failure()
        .stderr(predicates::str::contains("Auth, InvalidAction"));
}

#[tokio::test]
async fn non_root_auth_mode_signs_non_root_subcall() {
    let sandbox = &TestEnv::new();
    new_account(sandbox, "signer");

    let (id1, id2) = deploy_auth_contracts(sandbox).await;

    // with non-root auth mode, non-source signer, and auto-sign - expect success
    sandbox
        .new_assert_cmd("contract")
        .arg("invoke")
        .arg("--source=test")
        .arg("--id")
        .arg(&id1)
        .arg("--auth-mode=non-root")
        .arg("--auto-sign")
        .arg("--")
        .arg("no-auth-sub-auth")
        .arg("--addr=signer")
        .arg("--val=hello")
        .arg(&format!("--subcall={id2}"))
        .assert()
        .success()
        .stdout("\"hello\"\n");

    // with non-root auth mode, source signer, and no auto-sign - expect success
    // -> signature is covered by the envelope signature, no explicit signature needed
    sandbox
        .new_assert_cmd("contract")
        .arg("invoke")
        .arg("--source=test")
        .arg("--id")
        .arg(&id1)
        .arg("--auth-mode=non-root")
        .arg("--")
        .arg("no-auth-sub-auth")
        .arg("--addr=test")
        .arg("--val=hello")
        .arg(&format!("--subcall={id2}"))
        .assert()
        .success()
        .stdout("\"hello\"\n");
}

#[tokio::test]
async fn non_root_auth_mode_via_env_var() {
    let sandbox = &TestEnv::new();
    new_account(sandbox, "signer");

    let (id1, id2) = deploy_auth_contracts(sandbox).await;

    // `STELLAR_AUTH_MODE` is the env-var equivalent of `--auth-mode`.
    sandbox
        .new_assert_cmd("contract")
        .env("STELLAR_AUTH_MODE", "non-root")
        .arg("invoke")
        .arg("--source=test")
        .arg("--id")
        .arg(&id1)
        .arg("--auto-sign")
        .arg("--")
        .arg("no-auth-sub-auth")
        .arg("--addr=signer")
        .arg("--val=hello")
        .arg(&format!("--subcall={id2}"))
        .assert()
        .success()
        .stdout("\"hello\"\n");
}

#[tokio::test]
async fn partial_auth_with_authorized_subcall() {
    let sandbox = &TestEnv::new();
    new_account(sandbox, "signer");

    let (id1, id2) = deploy_auth_contracts(sandbox).await;

    // with non-source signer - expect failure
    sandbox
        .new_assert_cmd("contract")
        .arg("invoke")
        .arg("--source=test")
        .arg("--id")
        .arg(&id1)
        .arg("--")
        .arg("partial_auth_sub_auth")
        .arg("--addr=signer")
        .arg("--val=hello")
        .arg(&format!("--subcall={id2}"))
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "An authorization entry requires confirmation",
        ));

    // with non-source signer and --auto-sign - expect success
    sandbox
        .new_assert_cmd("contract")
        .arg("invoke")
        .arg("--source=test")
        .arg("--id")
        .arg(&id1)
        .arg("--auto-sign")
        .arg("--")
        .arg("partial_auth_sub_auth")
        .arg("--addr=signer")
        .arg("--val=hello")
        .arg(&format!("--subcall={id2}"))
        .assert()
        .success()
        .stdout("\"hello\"\n");

    // with source signer - expect success
    sandbox
        .new_assert_cmd("contract")
        .arg("invoke")
        .arg("--source=test")
        .arg("--id")
        .arg(&id1)
        .arg("--")
        .arg("partial_auth_sub_auth")
        .arg("--addr=test")
        .arg("--val=hello")
        .arg(&format!("--subcall={id2}"))
        .assert()
        .success()
        .stdout("\"hello\"\n");
}

#[tokio::test]
async fn constructor_auth_with_non_source_signer() {
    let sandbox = &TestEnv::new();
    new_account(sandbox, "signer");

    constructor_cmd(sandbox, "signer")
        .assert()
        .failure()
        .stderr(predicates::str::contains("Auth, InvalidAction"));
}
