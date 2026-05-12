use crate::integration::util::{deploy_contract, test_address, DeployOptions, HELLO_WORLD};

use soroban_test::{AssertExt, TestEnv};

#[tokio::test]
async fn info_hash_with_wasm_file() {
    let sandbox = &TestEnv::new();
    let expected = HELLO_WORLD.hash().unwrap().to_string();

    let actual = sandbox
        .new_assert_cmd("contract")
        .arg("info")
        .arg("hash")
        .arg("--wasm")
        .arg(HELLO_WORLD.path())
        .assert()
        .success()
        .stdout_as_str();

    assert_eq!(actual, expected);
}

#[tokio::test]
async fn info_hash_with_contract_id() {
    let sandbox = &TestEnv::new();
    let expected = HELLO_WORLD.hash().unwrap().to_string();
    let contract_id = deploy_contract(sandbox, HELLO_WORLD, DeployOptions::default()).await;

    let actual = sandbox
        .new_assert_cmd("contract")
        .arg("info")
        .arg("hash")
        .arg("--id")
        .arg(&contract_id)
        .assert()
        .success()
        .stdout_as_str();

    assert_eq!(actual, expected);
}

#[tokio::test]
async fn info_hash_with_contract_alias() {
    let sandbox = &TestEnv::new();
    let expected = HELLO_WORLD.hash().unwrap().to_string();
    let contract_id = deploy_contract(sandbox, HELLO_WORLD, DeployOptions::default()).await;

    sandbox
        .new_assert_cmd("contract")
        .arg("alias")
        .arg("add")
        .arg("hello")
        .arg("--id")
        .arg(&contract_id)
        .assert()
        .success();

    let actual = sandbox
        .new_assert_cmd("contract")
        .arg("info")
        .arg("hash")
        .arg("--id")
        .arg("hello")
        .assert()
        .success()
        .stdout_as_str();

    assert_eq!(actual, expected);
}

#[tokio::test]
async fn info_hash_errors_on_stellar_asset_contract() {
    let sandbox = &TestEnv::new();
    let issuer = test_address(sandbox);
    let sac_id = sandbox
        .new_assert_cmd("contract")
        .arg("asset")
        .arg("deploy")
        .arg(format!("--asset=USDC:{issuer}"))
        .assert()
        .success()
        .stdout_as_str();

    sandbox
        .new_assert_cmd("contract")
        .arg("info")
        .arg("hash")
        .arg("--id")
        .arg(&sac_id)
        .assert()
        .failure();
}

#[tokio::test]
async fn info_hash_requires_one_source() {
    let sandbox = &TestEnv::new();

    sandbox
        .new_assert_cmd("contract")
        .arg("info")
        .arg("hash")
        .assert()
        .failure();
}

#[tokio::test]
async fn info_hash_wasm_and_id_are_mutually_exclusive() {
    let sandbox = &TestEnv::new();
    let contract_id = deploy_contract(sandbox, HELLO_WORLD, DeployOptions::default()).await;

    sandbox
        .new_assert_cmd("contract")
        .arg("info")
        .arg("hash")
        .arg("--wasm")
        .arg(HELLO_WORLD.path())
        .arg("--id")
        .arg(&contract_id)
        .assert()
        .failure();
}
