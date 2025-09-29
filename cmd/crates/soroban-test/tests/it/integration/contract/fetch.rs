use crate::integration::util::{deploy_contract, DeployOptions, HELLO_WORLD};

use soroban_test::TestEnv;

#[tokio::test]
async fn tx_fetch_with_hash() {
    let sandbox = &TestEnv::new();
    let test_account_alias = "test";
    let wasm_bytes = HELLO_WORLD.bytes();
    let wasm_hash = HELLO_WORLD.hash().unwrap();
    let _contract_id = deploy_contract(
        sandbox,
        HELLO_WORLD,
        DeployOptions {
            deployer: Some(test_account_alias.to_string()),
            ..Default::default()
        },
    )
    .await;

    sandbox
        .new_assert_cmd("contract")
        .arg("fetch")
        .arg("--wasm-hash")
        .arg(wasm_hash.to_string())
        .assert()
        .success()
        .stdout(predicates::ord::eq(wasm_bytes));
}

#[tokio::test]
async fn tx_fetch_with_id() {
    let sandbox = &TestEnv::new();
    let test_account_alias = "test";
    let wasm_bytes = HELLO_WORLD.bytes();
    let contract_id = deploy_contract(
        sandbox,
        HELLO_WORLD,
        DeployOptions {
            deployer: Some(test_account_alias.to_string()),
            ..Default::default()
        },
    )
    .await;

    sandbox
        .new_assert_cmd("contract")
        .arg("fetch")
        .arg("--id")
        .arg(contract_id.clone())
        .assert()
        .success()
        .stdout(predicates::ord::eq(wasm_bytes));
}
