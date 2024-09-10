use soroban_cli::tx::{builder::String64, ONE_XLM};
use soroban_sdk::xdr::{self, ReadXdr, SequenceNumber};
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
    let test1_account_entry_before = client.get_account(&test1).await.unwrap();

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
    assert_eq!(
        ONE_XLM,
        test1_account_entry.balance - test1_account_entry_before.balance,
        "Should have One XLM more"
    );
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
    let fee = 100;
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "account-merge",
            "--source",
            "test1",
            "--account",
            test.as_str(),
            "--fee",
            fee.to_string().as_str(),
        ])
        .assert()
        .success();
    let after = client.get_account(&test).await.unwrap();
    assert!(client.get_account(&test1).await.is_err());
    assert_eq!(before.balance + before1.balance - fee, after.balance);
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

#[tokio::test]
async fn set_options() {
    let sandbox = &TestEnv::new();
    let client = soroban_rpc::Client::new(&sandbox.rpc_url).unwrap();
    let test = sandbox
        .new_assert_cmd("keys")
        .arg("address")
        .arg("test")
        .assert()
        .success()
        .stdout_as_str();
    let before = client.get_account(&test).await.unwrap();
    assert!(before.inflation_dest.is_none());
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "set-options",
            "--inflation-dest",
            test.as_str(),
            "--home-domain",
            "test.com",
        ])
        .assert()
        .success();
    let after = client.get_account(&test).await.unwrap();
    let xdr::PublicKey::PublicKeyTypeEd25519(xdr::Uint256(key)) = after.inflation_dest.unwrap().0;

    assert_eq!(test, stellar_strkey::ed25519::PublicKey(key).to_string());
    assert_eq!("test.com", after.home_domain.to_string());
}

#[tokio::test]
async fn change_trust() {
    let sandbox = &TestEnv::new();
    let asset = "native";
    let limit = 100;
    println!(
        "{}",
        sandbox
            .new_assert_cmd("tx")
            .args([
                "new",
                "change-trust",
                "--line",
                asset,
                "--limit",
                limit.to_string().as_str(),
                "--build-only",
            ])
            .assert()
            .success()
            .stdout_as_str()
    );
}

#[tokio::test]
async fn manage_data() {
    let sandbox = &TestEnv::new();
    let (test, _) = setup_accounts(sandbox);
    let client = soroban_rpc::Client::new(&sandbox.rpc_url).unwrap();
    let key = "test";
    let value = "beefface";
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "manage-data",
            "--data-name",
            key,
            "--data-value",
            value,
        ])
        .assert()
        .success();
    let account_id = xdr::AccountId(xdr::PublicKey::PublicKeyTypeEd25519(xdr::Uint256(
        stellar_strkey::ed25519::PublicKey::from_string(&test)
            .unwrap()
            .0,
    )));
    let orig_data_name: String64 = key.parse().unwrap();
    let res = client
        .get_ledger_entries(&[xdr::LedgerKey::Data(xdr::LedgerKeyData {
            account_id,
            data_name: orig_data_name.clone().into(),
        })])
        .await
        .unwrap();
    let value_res = res.entries.as_ref().unwrap().first().unwrap();
    let ledeger_entry_data =
        xdr::LedgerEntryData::from_xdr_base64(&value_res.xdr, xdr::Limits::none()).unwrap();
    let xdr::LedgerEntryData::Data(xdr::DataEntry {
        data_value,
        data_name,
        ..
    }) = ledeger_entry_data
    else {
        panic!("Expected DataEntry");
    };
    assert_eq!(data_name, orig_data_name.into());
    assert_eq!(hex::encode(data_value.0.to_vec()), value);
}
