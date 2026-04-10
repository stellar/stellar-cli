use soroban_test::TestEnv;

use super::util::{deploy_contract, extend_contract, new_account, DeployOptions, AUTH};

/// Helper to deploy two instances of the auth contract and extend them.
/// Returns (contract_id_1, contract_id_2).
async fn deploy_auth_contracts(sandbox: &TestEnv) -> (String, String) {
    let id1 = deploy_contract(
        sandbox,
        AUTH,
        DeployOptions {
            salt: Some("0000000000000000000000000000000000000000000000000000000000000001".into()),
            ..Default::default()
        },
    )
    .await;
    extend_contract(sandbox, &id1).await;

    let id2 = deploy_contract(
        sandbox,
        AUTH,
        DeployOptions {
            salt: Some("0000000000000000000000000000000000000000000000000000000000000002".into()),
            ..Default::default()
        },
    )
    .await;
    extend_contract(sandbox, &id2).await;

    (id1, id2)
}

#[tokio::test]
async fn standard_auth_with_separate_signer() {
    let sandbox = &TestEnv::new();
    let (id, _) = deploy_auth_contracts(sandbox).await;
    new_account(sandbox, "signer");

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
    let (id1, id2) = deploy_auth_contracts(sandbox).await;

    new_account(sandbox, "signer");

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
async fn non_root_auth_subcall_fails() {
    let sandbox = &TestEnv::new();
    let (id1, id2) = deploy_auth_contracts(sandbox).await;

    new_account(sandbox, "signer");

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
}

#[tokio::test]
async fn partial_auth_source_account_fails() {
    let sandbox = &TestEnv::new();
    let (id1, id2) = deploy_auth_contracts(sandbox).await;

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
        .failure()
        .stderr(predicates::str::contains("Auth, InvalidAction"));
}
