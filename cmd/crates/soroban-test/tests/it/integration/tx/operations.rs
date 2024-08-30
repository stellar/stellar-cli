use soroban_cli::tx::ONE_XLM;
use soroban_sdk::xdr::SequenceNumber;
use soroban_test::{AssertExt, TestEnv};

use crate::integration::{
    hello_world::invoke_hello_world,
    util::{deploy_contract, DeployKind, HELLO_WORLD},
};

fn test_address(sandbox: &TestEnv) -> String {
    sandbox
        .new_assert_cmd("keys")
        .arg("address")
        .arg("test")
        .assert()
        .success()
        .stdout_as_str()
}

// returns test and test1 addresses
fn setup_accounts(sandbox: &TestEnv) -> (String, String) {
    let test = test_address(sandbox);
    sandbox.generate_account("test1", None).assert().success();
    let test1 = sandbox
        .new_assert_cmd("keys")
        .arg("address")
        .arg("test1")
        .assert()
        .success()
        .stdout_as_str();
    (test, test1)
}

#[tokio::test]
async fn create_account() {
    let sandbox = &TestEnv::new();
    sandbox
        .new_assert_cmd("keys")
        .args(["generate", "--no-fund", "new"])
        .assert()
        .success();

    let address = sandbox
        .new_assert_cmd("keys")
        .args(["address", "new"])
        .assert()
        .success()
        .stdout_as_str();
    let test = test_address(sandbox);
    let client = soroban_rpc::Client::new(&sandbox.rpc_url).unwrap();
    let test_account = client.get_account(&test).await.unwrap();
    println!("test account has a balance of {}", test_account.balance);
    let starting_balance = ONE_XLM * 100;
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "create-account",
            "--destination",
            address.as_str(),
            "--starting-balance",
            starting_balance.to_string().as_str(),
        ])
        .assert()
        .success();
    let test_account_after = client.get_account(&test).await.unwrap();
    assert!(test_account_after.balance < test_account.balance);
    let id = deploy_contract(sandbox, HELLO_WORLD, DeployKind::Normal, Some("new")).await;
    println!("{id}");
    invoke_hello_world(sandbox, &id);
}

#[tokio::test]
async fn payment() {
    let sandbox = &TestEnv::new();
    let client = soroban_rpc::Client::new(&sandbox.rpc_url).unwrap();
    let (test, test1) = setup_accounts(sandbox);
    let test_account = client.get_account(&test).await.unwrap();
    println!("test account has a balance of {}", test_account.balance);

    let before = client.get_account(&test).await.unwrap();
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "payment",
            "--destination",
            test1.as_str(),
            "--amount",
            ONE_XLM.to_string().as_str(),
        ])
        .assert()
        .success();
    let test1_account_entry = client.get_account(&test1).await.unwrap();
    assert_eq!(ONE_XLM, test1_account_entry.balance);
    let after = client.get_account(&test).await.unwrap();
    assert_eq!(before.balance - 10_000_100, after.balance);
}

#[tokio::test]
async fn bump_sequence() {
    let sandbox = &TestEnv::new();
    let client = soroban_rpc::Client::new(&sandbox.rpc_url).unwrap();
    let test = test_address(sandbox);
    let before = client.get_account(&test).await.unwrap();
    let amount = 50;
    let seq = SequenceNumber(before.seq_num.0 + amount);
    // bump sequence tx new
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "bump-sequence",
            "--bump-to",
            seq.0.to_string().as_str(),
        ])
        .assert()
        .success();
    let after = client.get_account(&test).await.unwrap();
    assert_eq!(seq, after.seq_num);
}

#[tokio::test]
async fn account_merge() {
    let sandbox = &TestEnv::new();
    let client = soroban_rpc::Client::new(&sandbox.rpc_url).unwrap();
    let (test, test1) = setup_accounts(sandbox);
    let before = client.get_account(&test).await.unwrap();
    let before1 = client.get_account(&test1).await.unwrap();
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "account-merge",
            "--source",
            test1.as_str(),
            "--destination",
            test.as_str(),
        ])
        .assert()
        .success();
    let after = client.get_account(&test).await.unwrap();
    let after1 = client.get_account(&test1).await.unwrap();
    assert_eq!(before.balance + before1.balance, after.balance);
    assert_eq!(0, after1.balance);
}

#[tokio::test]
async fn set_trustline_flags() {
    let sandbox = &TestEnv::new();
    let client = soroban_rpc::Client::new(&sandbox.rpc_url).unwrap();
    let test = test_address(sandbox);
    let _ = client.get_account(&test).await.unwrap();
}

#[tokio::test]
async fn set_options_add_signer() {
    let sandbox = &TestEnv::new();
    let client = soroban_rpc::Client::new(&sandbox.rpc_url).unwrap();
    let (test, test1) = setup_accounts(sandbox);
    let before = client.get_account(&test).await.unwrap();
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "set-options",
            "--signer",
            test1.as_str(),
            "--signer-weight",
            "1",
        ])
        .assert()
        .success();
    let after = client.get_account(&test).await.unwrap();
    assert_eq!(before.signers.len() + 1, after.signers.len());
    // Now remove signer with a weight of 0
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "set-options",
            "--signer",
            test1.as_str(),
            "--signer-weight",
            "0",
        ])
        .assert()
        .success();
    let after = client.get_account(&test).await.unwrap();
    assert_eq!(before.signers.len(), after.signers.len());
}

// #[tokio::test]
// async fn set_options() {
//     let sandbox = &TestEnv::new();
//     let client = soroban_rpc::Client::new(&sandbox.rpc_url).unwrap();
//     let test = sandbox
//         .new_assert_cmd("keys")
//         .arg("address")
//         .arg("test")
//         .assert()
//         .success()
//         .stdout_as_str();
//     let before = client.get_account(&test).await.unwrap();
//     sandbox
//         .new_assert_cmd("tx")
//         .args([
//             "new",
//             "set-options",
//             "--inflation-destination",
//             test.as_str(),
//             "--home-domain",
//             "test.com",
//         ])
//         .assert()
//         .success();
//     let after = client.get_account(&test).await.unwrap();
//     assert_eq!(test, after.inflation_destination.unwrap());
//     assert_eq!("test.com", after.home_domain.unwrap());
// }
