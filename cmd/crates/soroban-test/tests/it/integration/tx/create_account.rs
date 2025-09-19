use soroban_cli::{
    tx::ONE_XLM,
    xdr::{self, ReadXdr},
};
use soroban_rpc::LedgerEntryResult;
use soroban_test::{AssertExt, TestEnv};

use crate::integration::{
    hello_world::invoke_hello_world,
    util::{deploy_contract, gen_account_no_fund, test_address, DeployOptions, HELLO_WORLD},
};

#[tokio::test]
async fn create_account() {
    let sandbox = &TestEnv::new();
    sandbox
        .new_assert_cmd("keys")
        .args(["generate", "new"])
        .assert()
        .success();

    let address = sandbox
        .new_assert_cmd("keys")
        .args(["address", "new"])
        .assert()
        .success()
        .stdout_as_str();
    let test = test_address(sandbox);
    let client = sandbox.network.rpc_client().unwrap();
    let test_account = client.get_account(&test).await.unwrap();
    println!("test account has a balance of {}", test_account.balance);
    let starting_balance = ONE_XLM * 5000; // 500 XLM to ensure enough for contract deployment
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
    let id = deploy_contract(
        sandbox,
        HELLO_WORLD,
        DeployOptions {
            deployer: Some("new".to_string()),
            ..Default::default()
        },
    )
    .await;
    println!("{id}");
    invoke_hello_world(sandbox, &id);
}

#[tokio::test]
async fn create_account_with_alias() {
    let sandbox = &TestEnv::new();
    sandbox
        .new_assert_cmd("keys")
        .args(["generate", "new"])
        .assert()
        .success();
    let test = test_address(sandbox);
    let client = sandbox.client();
    let test_account = client.get_account(&test).await.unwrap();
    println!("test account has a balance of {}", test_account.balance);
    let starting_balance = ONE_XLM * 5000; // 500 XLM to ensure enough for contract deployment
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "create-account",
            "--destination",
            "new",
            "--starting-balance",
            starting_balance.to_string().as_str(),
        ])
        .assert()
        .success();
    let test_account_after = client.get_account(&test).await.unwrap();
    assert!(test_account_after.balance < test_account.balance);
    let id = deploy_contract(
        sandbox,
        HELLO_WORLD,
        DeployOptions {
            deployer: Some("new".to_string()),
            ..Default::default()
        },
    )
    .await;
    println!("{id}");
    invoke_hello_world(sandbox, &id);
}

#[tokio::test]
async fn multi_create_accounts() {
    let sandbox = &TestEnv::new();
    let client = sandbox.network.rpc_client().unwrap();
    let nums: Vec<u8> = (1..=3).collect();
    let mut accounts: Vec<(String, String)> = nums
        .iter()
        .map(|x| {
            let name = format!("test_{x}");
            let address = gen_account_no_fund(sandbox, &name);
            (name, address)
        })
        .collect();
    let (_, test_99_address) = accounts.pop().unwrap();

    let input = sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "create-account",
            "--fee=1000000",
            "--build-only",
            "--destination",
            &test_99_address,
        ])
        .assert()
        .success()
        .stdout_as_str();

    let final_tx = accounts.iter().fold(input, |tx_env, (_, address)| {
        sandbox
            .new_assert_cmd("tx")
            .args(["op", "add", "create-account", "--destination", address])
            .write_stdin(tx_env.as_bytes())
            .assert()
            .success()
            .stdout_as_str()
    });
    let out = sandbox
        .new_assert_cmd("tx")
        .arg("send")
        .write_stdin(
            sandbox
                .new_assert_cmd("tx")
                .arg("sign")
                .arg("--sign-with-key=test")
                .write_stdin(final_tx.as_bytes())
                .assert()
                .success()
                .stdout_as_str()
                .as_bytes(),
        )
        .assert()
        .success()
        .stdout_as_str();
    println!("{out}");
    let keys = accounts
        .iter()
        .map(|(_, address)| {
            xdr::LedgerKey::Account(xdr::LedgerKeyAccount {
                account_id: address.parse().unwrap(),
            })
        })
        .collect::<Vec<_>>();

    let account = client.get_account(&test_99_address).await.unwrap();
    println!("{account:#?}");
    let entries = client.get_ledger_entries(&keys).await.unwrap();
    println!("{entries:#?}");
    entries
        .entries
        .unwrap()
        .iter()
        .for_each(|LedgerEntryResult { xdr, .. }| {
            let xdr::LedgerEntryData::Account(value) =
                xdr::LedgerEntryData::from_xdr_base64(xdr, xdr::Limits::none()).unwrap()
            else {
                panic!("Expected Account");
            };
            assert_eq!(value.balance, 10_000_000);
        });
}
