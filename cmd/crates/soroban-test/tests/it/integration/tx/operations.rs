use soroban_cli::{
    config::locator,
    tx::{builder, ONE_XLM},
    utils::contract_id_hash_from_asset,
    xdr::{self, ReadXdr, SequenceNumber},
};

use soroban_rpc::LedgerEntryResult;
use soroban_test::{AssertExt, TestEnv};

use crate::integration::{
    hello_world::invoke_hello_world,
    util::{deploy_contract, test_address, DeployOptions, HELLO_WORLD},
};

fn new_account(sandbox: &TestEnv, name: &str) -> String {
    sandbox.generate_account(name, None).assert().success();
    sandbox
        .new_assert_cmd("keys")
        .args(["address", name])
        .assert()
        .success()
        .stdout_as_str()
}

fn gen_account_no_fund(sandbox: &TestEnv, name: &str) -> String {
    sandbox
        .new_assert_cmd("keys")
        .args(["generate", name])
        .assert()
        .success();
    sandbox
        .new_assert_cmd("keys")
        .args(["address", name])
        .assert()
        .success()
        .stdout_as_str()
}

// returns test and test1 addresses
fn setup_accounts(sandbox: &TestEnv) -> (String, String) {
    (test_address(sandbox), new_account(sandbox, "test1"))
}

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
async fn payment_with_alias() {
    let sandbox = &TestEnv::new();
    let client = sandbox.client();
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
            "test1",
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
async fn payment() {
    let sandbox = &TestEnv::new();
    let client = sandbox.network.rpc_client().unwrap();
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
            "10_000_000",
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
    let client = sandbox.network.rpc_client().unwrap();
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
    let client = sandbox.network.rpc_client().unwrap();
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
async fn account_merge_with_alias() {
    let sandbox = &TestEnv::new();
    let client = sandbox.client();
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
            "test",
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
    let (test, test1_address) = setup_accounts(sandbox);
    let asset = "usdc:test1";
    issue_asset(sandbox, &test, asset, 100_000, 100).await;
    sandbox
        .new_assert_cmd("contract")
        .arg("asset")
        .arg("deploy")
        .arg("--asset")
        .arg(asset)
        .assert()
        .success();
    let id = contract_id_hash_from_asset(
        &format!("usdc:{test1_address}")
            .parse::<builder::Asset>()
            .unwrap()
            .resolve(&locator::Args::default())
            .unwrap(),
        &sandbox.network.network_passphrase,
    );
    // sandbox
    //     .new_assert_cmd("contract")
    //     .args([
    //         "invoke",
    //         "--id",
    //         &id.to_string(),
    //         "--",
    //         "authorized",
    //         "--id",
    //         &test,
    //     ])
    //     .assert()
    //     .success()
    //     .stdout("false\n");

    sandbox
        .new_assert_cmd("contract")
        .args([
            "invoke",
            "--id",
            &id.to_string(),
            "--",
            "authorized",
            "--id",
            &test,
        ])
        .assert()
        .success()
        .stdout("true\n");
}

#[tokio::test]
async fn set_options_add_signer() {
    let sandbox = &TestEnv::new();
    let client = sandbox.network.rpc_client().unwrap();
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
    assert_eq!(after.signers.first().unwrap().key, test1.parse().unwrap());
    let key = xdr::LedgerKey::Account(xdr::LedgerKeyAccount {
        account_id: test.parse().unwrap(),
    });
    let res = client.get_ledger_entries(&[key]).await.unwrap();
    let xdr_str = res.entries.unwrap().clone().first().unwrap().clone().xdr;
    let entry = xdr::LedgerEntryData::from_xdr_base64(&xdr_str, xdr::Limits::none()).unwrap();
    let xdr::LedgerEntryData::Account(xdr::AccountEntry { signers, .. }) = entry else {
        panic!();
    };
    assert_eq!(signers.first().unwrap().key, test1.parse().unwrap());

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

fn build_and_run(sandbox: &TestEnv, cmd: &str, args: &[&str]) -> String {
    let mut args_2 = args.to_vec();
    args_2.push("--build-only");
    let res = sandbox
        .new_assert_cmd(cmd)
        .args(args_2)
        .assert()
        .success()
        .stdout_as_str();
    sandbox.new_assert_cmd(cmd).args(args).assert().success();
    res
}

#[tokio::test]
async fn set_options() {
    let sandbox = &TestEnv::new();
    let client = sandbox.network.rpc_client().unwrap();
    let (test, alice) = setup_accounts(sandbox);
    let before = client.get_account(&test).await.unwrap();
    assert!(before.inflation_dest.is_none());
    let tx_xdr = build_and_run(
        sandbox,
        "tx",
        &[
            "new",
            "set-options",
            "--inflation-dest",
            alice.as_str(),
            "--home-domain",
            "test.com",
            "--master-weight=100",
            "--med-threshold=100",
            "--low-threshold=100",
            "--high-threshold=100",
            "--signer",
            alice.as_str(),
            "--signer-weight=100",
            "--set-required",
            "--set-revocable",
            "--set-clawback-enabled",
            "--set-immutable",
        ],
    );
    println!("{tx_xdr}");
    let after = client.get_account(&test).await.unwrap();
    println!("{before:#?}\n{after:#?}");
    assert_eq!(
        after.flags,
        xdr::AccountFlags::ClawbackEnabledFlag as u32
            | xdr::AccountFlags::ImmutableFlag as u32
            | xdr::AccountFlags::RevocableFlag as u32
            | xdr::AccountFlags::RequiredFlag as u32
    );
    assert_eq!([100, 100, 100, 100], after.thresholds.0);
    assert_eq!(100, after.signers[0].weight);
    assert_eq!(alice, after.signers[0].key.to_string());
    let xdr::PublicKey::PublicKeyTypeEd25519(xdr::Uint256(key)) = after.inflation_dest.unwrap().0;
    assert_eq!(alice, stellar_strkey::ed25519::PublicKey(key).to_string());
    assert_eq!("test.com", after.home_domain.to_string());
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "set-options",
            "--inflation-dest",
            test.as_str(),
            "--home-domain",
            "test.com",
            "--master-weight=100",
            "--med-threshold=100",
            "--low-threshold=100",
            "--high-threshold=100",
            "--signer",
            alice.as_str(),
            "--signer-weight=100",
            "--set-required",
            "--set-revocable",
            "--set-clawback-enabled",
        ])
        .assert()
        .failure();
}

#[tokio::test]
async fn set_some_options() {
    let sandbox = &TestEnv::new();
    let client = sandbox.network.rpc_client().unwrap();
    let test = test_address(sandbox);
    let before = client.get_account(&test).await.unwrap();
    assert!(before.inflation_dest.is_none());
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "set-options",
            "--set-clawback-enabled",
            "--master-weight=100",
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains("AuthRevocableRequired"));
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "set-options",
            "--set-revocable",
            "--master-weight=100",
        ])
        .assert()
        .success();
    let after = client.get_account(&test).await.unwrap();
    assert_eq!(after.flags, xdr::AccountFlags::RevocableFlag as u32);
    assert_eq!([100, 0, 0, 0], after.thresholds.0);
    assert!(after.inflation_dest.is_none());
    assert_eq!(
        after.home_domain,
        "".parse::<xdr::StringM<32>>().unwrap().into()
    );
    assert!(after.signers.is_empty());
    sandbox
        .new_assert_cmd("tx")
        .args(["new", "set-options", "--set-clawback-enabled"])
        .assert()
        .success();
    let after = client.get_account(&test).await.unwrap();
    assert_eq!(
        after.flags,
        xdr::AccountFlags::RevocableFlag as u32 | xdr::AccountFlags::ClawbackEnabledFlag as u32
    );
    sandbox
        .new_assert_cmd("tx")
        .args(["new", "set-options", "--clear-clawback-enabled"])
        .assert()
        .success();
    let after = client.get_account(&test).await.unwrap();
    assert_eq!(after.flags, xdr::AccountFlags::RevocableFlag as u32);
    sandbox
        .new_assert_cmd("tx")
        .args(["new", "set-options", "--clear-revocable"])
        .assert()
        .success();
    let after = client.get_account(&test).await.unwrap();
    assert_eq!(after.flags, 0);
    sandbox
        .new_assert_cmd("tx")
        .args(["new", "set-options", "--set-required"])
        .assert()
        .success();
    let after = client.get_account(&test).await.unwrap();
    assert_eq!(after.flags, xdr::AccountFlags::RequiredFlag as u32);
    sandbox
        .new_assert_cmd("tx")
        .args(["new", "set-options", "--clear-required"])
        .assert()
        .success();
    let after = client.get_account(&test).await.unwrap();
    assert_eq!(after.flags, 0);
}

#[tokio::test]
async fn change_trust() {
    let sandbox = &TestEnv::new();
    let (test, issuer) = setup_accounts(sandbox);
    let asset = &format!("usdc:{issuer}");

    let limit = 100_000_000;
    let half_limit = limit / 2;
    issue_asset(sandbox, &test, asset, limit, half_limit).await;
    sandbox
        .new_assert_cmd("contract")
        .arg("asset")
        .arg("deploy")
        .arg("--asset")
        .arg(asset)
        .assert()
        .success();

    // wrap_cmd(&asset).run().await.unwrap();
    let id = contract_id_hash_from_asset(
        &asset
            .parse::<builder::Asset>()
            .unwrap()
            .resolve(&locator::Args::default())
            .unwrap(),
        &sandbox.network.network_passphrase,
    );
    sandbox
        .new_assert_cmd("contract")
        .args([
            "invoke",
            "--id",
            &id.to_string(),
            "--",
            "balance",
            "--id",
            &test,
        ])
        .assert()
        .stdout(format!("\"{half_limit}\"\n"));

    let bob = new_account(sandbox, "bob");
    let bobs_limit = half_limit / 2;
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "change-trust",
            "--source=bob",
            "--line",
            asset,
            "--limit",
            bobs_limit.to_string().as_str(),
        ])
        .assert()
        .success();
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "payment",
            "--destination",
            &bob,
            "--asset",
            asset,
            "--amount",
            half_limit.to_string().as_str(),
        ])
        .assert()
        .failure();
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "payment",
            "--destination",
            &bob,
            "--asset",
            asset,
            "--amount",
            bobs_limit.to_string().as_str(),
        ])
        .assert()
        .success();
    sandbox
        .new_assert_cmd("contract")
        .args([
            "invoke",
            "--id",
            &id.to_string(),
            "--",
            "balance",
            "--id",
            &bob,
        ])
        .assert()
        .stdout(format!("\"{bobs_limit}\"\n"));
}

#[tokio::test]
async fn manage_data() {
    let sandbox = &TestEnv::new();
    let (test, _) = setup_accounts(sandbox);
    let client = sandbox.network.rpc_client().unwrap();
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
    let orig_data_name: xdr::StringM<64> = key.parse().unwrap();
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

async fn issue_asset(sandbox: &TestEnv, test: &str, asset: &str, limit: u64, initial_balance: u64) {
    let client = sandbox.network.rpc_client().unwrap();
    let test_before = client.get_account(test).await.unwrap();
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "change-trust",
            "--line",
            asset,
            "--limit",
            limit.to_string().as_str(),
        ])
        .assert()
        .success();

    sandbox
        .new_assert_cmd("tx")
        .args(["new", "set-options", "--set-required"])
        .assert()
        .success();
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "set-trustline-flags",
            "--asset",
            asset,
            "--trustor",
            test,
            "--set-authorize",
            "--source",
            "test1",
        ])
        .assert()
        .success();

    let after = client.get_account(test).await.unwrap();
    assert_eq!(test_before.num_sub_entries + 1, after.num_sub_entries);
    println!("aa");
    // Send a payment to the issuer
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "payment",
            "--destination",
            test,
            "--asset",
            asset,
            "--amount",
            initial_balance.to_string().as_str(),
            "--source=test1",
        ])
        .assert()
        .success();
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

#[tokio::test]
async fn manage_sell_offer() {
    let sandbox = &TestEnv::new();
    let client = sandbox.network.rpc_client().unwrap();
    let (test, issuer) = setup_accounts(sandbox);
    let asset = format!("USD:{issuer}");

    // Create trustline and issue some USD to the test account
    let limit = 100_000_000_000; // 10,000 USD
    let initial_balance = 50_000_000_000; // 5,000 USD
    issue_asset(sandbox, &test, &asset, limit, initial_balance).await;

    let test_account_before = client.get_account(&test).await.unwrap();

    // Create a new sell offer: sell 1000 USD for XLM at price 1:2 (0.5 USD per XLM)
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "manage-sell-offer",
            "--selling",
            &asset,
            "--buying",
            "native",
            "--amount",
            "10000000000", // 1000 USD in stroops
            "--price",
            "1:2", // 0.5 USD per XLM
        ])
        .assert()
        .success();

    let test_account_after = client.get_account(&test).await.unwrap();

    // Account should have one more sub-entry (the offer)
    assert_eq!(
        test_account_before.num_sub_entries + 1,
        test_account_after.num_sub_entries,
        "Should have one additional sub-entry for the offer"
    );
}

#[tokio::test]
async fn manage_buy_offer() {
    let sandbox = &TestEnv::new();
    let client = sandbox.network.rpc_client().unwrap();
    let (test, issuer) = setup_accounts(sandbox);
    let asset = format!("EUR:{issuer}");

    // Create trustline and issue some EUR to the test account
    let limit = 100_000_000_000; // 10,000 EUR
    let initial_balance = 50_000_000_000; // 5,000 EUR
    issue_asset(sandbox, &test, &asset, limit, initial_balance).await;

    let test_account_before = client.get_account(&test).await.unwrap();

    // Create a new buy offer: buy 1000 EUR with XLM at price 2:1 (2 XLM per EUR)
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "manage-buy-offer",
            "--selling",
            "native",
            "--buying",
            &asset,
            "--amount",
            "10000000000", // 1000 EUR in stroops
            "--price",
            "2:1", // 2 XLM per EUR
        ])
        .assert()
        .success();

    let test_account_after = client.get_account(&test).await.unwrap();

    // Account should have one more sub-entry (the offer)
    assert_eq!(
        test_account_before.num_sub_entries + 1,
        test_account_after.num_sub_entries,
        "Should have one additional sub-entry for the offer"
    );
}

#[tokio::test]
async fn create_passive_sell_offer() {
    let sandbox = &TestEnv::new();
    let client = sandbox.network.rpc_client().unwrap();
    let (test, issuer) = setup_accounts(sandbox);
    let asset = format!("JPY:{issuer}");

    // Create trustline and issue some JPY to the test account
    let limit = 100_000_000_000; // 10,000 JPY
    let initial_balance = 50_000_000_000; // 5,000 JPY
    issue_asset(sandbox, &test, &asset, limit, initial_balance).await;

    let test_account_before = client.get_account(&test).await.unwrap();

    // Create a passive sell offer: sell 1000 JPY for XLM at price 1:3 (0.33 JPY per XLM)
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "create-passive-sell-offer",
            "--selling",
            &asset,
            "--buying",
            "native",
            "--amount",
            "10000000000", // 1000 JPY in stroops
            "--price",
            "1:3", // 0.33 JPY per XLM
        ])
        .assert()
        .success();

    let test_account_after = client.get_account(&test).await.unwrap();

    // Account should have one more sub-entry (the passive offer)
    assert_eq!(
        test_account_before.num_sub_entries + 1,
        test_account_after.num_sub_entries,
        "Should have one additional sub-entry for the passive offer"
    );
}

#[tokio::test]
async fn path_payment_strict_send() {
    let sandbox = &TestEnv::new();
    let (test, issuer) = setup_accounts(sandbox);

    // Create recipient account
    let recipient = new_account(sandbox, "recipient");

    // Create market maker account that will provide liquidity
    let market_maker = new_account(sandbox, "market_maker");

    // Create USD asset issued by the issuer (test1)
    let usd_asset = format!("USD:{issuer}");

    let limit = 100_000_000_000; // 10,000 units
    let initial_balance = 50_000_000_000; // 5,000 units

    // Setup trustlines and issue USD to test account
    issue_asset(sandbox, &test, &usd_asset, limit, initial_balance).await;

    // Setup trustlines and issue USD to market maker
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "change-trust",
            "--source",
            "market_maker",
            "--line",
            &usd_asset,
        ])
        .assert()
        .success();

    // Authorize market maker's trustline
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "set-trustline-flags",
            "--asset",
            &usd_asset,
            "--trustor",
            &market_maker,
            "--set-authorize",
            "--source",
            "test1",
        ])
        .assert()
        .success();

    // Issue USD to market maker
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "payment",
            "--destination",
            &market_maker,
            "--asset",
            &usd_asset,
            "--amount",
            initial_balance.to_string().as_str(),
            "--source=test1",
        ])
        .assert()
        .success();

    // Setup trustlines for recipient account
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "change-trust",
            "--source",
            "recipient",
            "--line",
            &usd_asset,
        ])
        .assert()
        .success();

    // Authorize recipient's trustline
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "set-trustline-flags",
            "--asset",
            &usd_asset,
            "--trustor",
            &recipient,
            "--set-authorize",
            "--source",
            "test1",
        ])
        .assert()
        .success();

    // Market maker creates a sell offer: sell USD for XLM at 1:2 ratio
    // This provides liquidity for XLM -> USD path payments
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "manage-sell-offer",
            "--source",
            "market_maker",
            "--selling",
            &usd_asset,
            "--buying",
            "native",
            "--amount",
            "10000000000", // 1,000 USD for sale (within the 5,000 USD balance)
            "--price",
            "1:2", // 1 USD = 2 XLM (USD is worth more than XLM)
        ])
        .assert()
        .success();

    let client = sandbox.network.rpc_client().unwrap();
    let test_balance_before = client.get_account(&test).await.unwrap().balance;
    let recipient_balance_before = client.get_account(&recipient).await.unwrap().balance;

    // Test path-payment-strict-send (send exactly 10 XLM, receive variable USD)
    // At 1 USD = 2 XLM, 10 XLM should get us about 5 USD
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "path-payment-strict-send",
            "--send-asset",
            "native",
            "--send-amount",
            "100000000", // Send exactly 10 XLM
            "--destination",
            &recipient,
            "--dest-asset",
            &usd_asset,
            "--dest-min",
            "40000000", // Minimum 4 USD to receive (allowing for spread)
        ])
        .assert()
        .success();

    // Verify balances changed as expected
    let test_balance_after = client.get_account(&test).await.unwrap().balance;
    let recipient_balance_after = client.get_account(&recipient).await.unwrap().balance;

    // Test account should have less XLM (sent 10 XLM + fees)
    assert!(
        test_balance_after < test_balance_before,
        "Test account should have less XLM after sending"
    );
    let xlm_sent = test_balance_before - test_balance_after;
    assert!(xlm_sent >= 100000000, "Should have sent at least 10 XLM");

    // Recipient account balance should be the same (they received USD, not XLM)
    assert_eq!(
        recipient_balance_after, recipient_balance_before,
        "Recipient XLM balance should be unchanged"
    );
}

#[tokio::test]
async fn path_payment_strict_receive() {
    let sandbox = &TestEnv::new();
    let (test, issuer) = setup_accounts(sandbox);

    // Create recipient account
    let recipient = new_account(sandbox, "recipient");

    // Create market maker account that will provide liquidity
    let market_maker = new_account(sandbox, "market_maker");

    // Create USD asset issued by the issuer (test1)
    let usd_asset = format!("USD:{issuer}");

    let limit = 100_000_000_000; // 10,000 units
    let initial_balance = 50_000_000_000; // 5,000 units

    // Setup trustlines and issue USD to test account
    issue_asset(sandbox, &test, &usd_asset, limit, initial_balance).await;

    // Setup trustlines and issue USD to market maker
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "change-trust",
            "--source",
            "market_maker",
            "--line",
            &usd_asset,
        ])
        .assert()
        .success();

    // Authorize market maker's trustline
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "set-trustline-flags",
            "--asset",
            &usd_asset,
            "--trustor",
            &market_maker,
            "--set-authorize",
            "--source",
            "test1",
        ])
        .assert()
        .success();

    // Issue USD to market maker
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "payment",
            "--destination",
            &market_maker,
            "--asset",
            &usd_asset,
            "--amount",
            initial_balance.to_string().as_str(),
            "--source=test1",
        ])
        .assert()
        .success();

    // Market maker creates a buy offer: buy USD with XLM at 2:1 ratio
    // This provides liquidity for USD -> XLM path payments
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "manage-buy-offer",
            "--source",
            "market_maker",
            "--selling",
            "native",
            "--buying",
            &usd_asset,
            "--amount",
            "1000000000", // Want to buy 100 USD
            "--price",
            "2:1", // 2 XLM = 1 USD
        ])
        .assert()
        .success();

    let client = sandbox.network.rpc_client().unwrap();
    let test_balance_before = client.get_account(&test).await.unwrap().balance;
    let recipient_balance_before = client.get_account(&recipient).await.unwrap().balance;

    // Test path-payment-strict-receive (send variable USD, receive exactly 4 XLM)
    // At 2 XLM = 1 USD, we need about 2 USD to get 4 XLM
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "path-payment-strict-receive",
            "--send-asset",
            &usd_asset,
            "--send-max",
            "300000000", // Max 30 USD to send (generous buffer)
            "--destination",
            &recipient,
            "--dest-asset",
            "native",
            "--dest-amount",
            "40000000", // Exactly 4 XLM to receive
        ])
        .assert()
        .success();

    // Verify balances changed as expected
    let test_balance_after = client.get_account(&test).await.unwrap().balance;
    let recipient_balance_after = client.get_account(&recipient).await.unwrap().balance;

    // Test account balance should be slightly lower due to fees (they sent USD, not XLM)
    assert!(
        test_balance_after <= test_balance_before,
        "Test account balance should be same or slightly lower due to fees"
    );

    // Recipient should have received exactly 4 XLM
    let xlm_received = recipient_balance_after - recipient_balance_before;
    assert_eq!(
        xlm_received, 40000000,
        "Recipient should have received exactly 4 XLM"
    );
}

#[tokio::test]
async fn create_claimable_balance() {
    let sandbox = &TestEnv::new();
    let client = sandbox.network.rpc_client().unwrap();
    let (test, _) = setup_accounts(sandbox);

    // Create claimant account
    let claimant = new_account(sandbox, "claimant");

    let test_balance_before = client.get_account(&test).await.unwrap().balance;

    // Create a claimable balance with unconditional predicate (default)
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "create-claimable-balance",
            "--asset",
            "native",
            "--amount",
            "100000000", // 10 XLM
            "--claimant",
            &claimant,
        ])
        .assert()
        .success();

    // Test with time-based predicate
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "create-claimable-balance",
            "--asset",
            "native",
            "--amount",
            "50000000", // 5 XLM
            "--claimant",
            &claimant,
        ])
        .assert()
        .success();

    let test_balance_after = client.get_account(&test).await.unwrap().balance;

    // Account balance should be lower due to creating claimable balances + fees
    assert!(
        test_balance_after < test_balance_before,
        "Test account should have less XLM after creating claimable balances"
    );

    let xlm_spent = test_balance_before - test_balance_after;
    assert!(
        xlm_spent >= 150000000, // At least 15 XLM for both claimable balances
        "Should have spent at least 15 XLM for claimable balances"
    );
}

#[tokio::test]
async fn clawback_claimable_balance() {
    let sandbox = &TestEnv::new();
    let (test, issuer) = setup_accounts(sandbox);

    // Enable revocable flag first, then clawback on the issuer account
    sandbox
        .new_assert_cmd("tx")
        .args(["new", "set-options", "--set-revocable", "--source", "test1"])
        .assert()
        .success();

    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "set-options",
            "--set-clawback-enabled",
            "--source",
            "test1",
        ])
        .assert()
        .success();

    // Create asset for claimable balance
    let asset = format!("USDC:{issuer}");
    let limit = 100_000_000_000;
    let initial_balance = 50_000_000_000;
    issue_asset(sandbox, &test, &asset, limit, initial_balance).await;

    // Create claimant account
    let claimant = new_account(sandbox, "claimant");

    // Setup trustline for claimant
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "change-trust",
            "--source",
            "claimant",
            "--line",
            &asset,
        ])
        .assert()
        .success();

    // Authorize claimant's trustline
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "set-trustline-flags",
            "--asset",
            &asset,
            "--trustor",
            &claimant,
            "--set-authorize",
            "--source",
            "test1",
        ])
        .assert()
        .success();

    // Create a claimable balance with the USDC asset
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "create-claimable-balance",
            "--asset",
            &asset,
            "--amount",
            "10000000000", // 1000 USDC
            "--claimant",
            &claimant,
        ])
        .assert()
        .success();

    // Fetch the balance ID from Horizon
    let horizon_url = format!(
        "http://localhost:8000/claimable_balances/?claimant={}",
        claimant
    );
    let response = reqwest::get(&horizon_url)
        .await
        .expect("Failed to fetch claimable balances from Horizon");

    let json: serde_json::Value = response
        .json()
        .await
        .expect("Failed to parse Horizon response");

    // Extract the balance ID from the response
    let balance_id = json["_embedded"]["records"][0]["id"]
        .as_str()
        .expect("Failed to get balance ID from Horizon response");

    // Test clawback-claimable-balance command
    // this should succeed for the issuer
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "clawback-claimable-balance",
            "--balance-id",
            balance_id,
            "--source",
            "test1", // issuer should be able to clawback
        ])
        .assert()
        .success();

    // Verify the claimable balance can no longer be claimed after clawback
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "claim-claimable-balance",
            "--balance-id",
            balance_id,
            "--source",
            "claimant", // claimant should no longer be able to claim
        ])
        .assert()
        .failure(); // This should fail because the balance was clawed back
}

#[tokio::test]
async fn clawback() {
    let sandbox = &TestEnv::new();
    let (test, issuer) = setup_accounts(sandbox);

    // Enable revocable flag first, then clawback on the issuer account
    sandbox
        .new_assert_cmd("tx")
        .args(["new", "set-options", "--set-revocable", "--source", "test1"])
        .assert()
        .success();

    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "set-options",
            "--set-clawback-enabled",
            "--source",
            "test1",
        ])
        .assert()
        .success();

    // Create asset for clawback test
    let asset = format!("USDC:{issuer}");
    let limit = 100_000_000_000;
    let initial_balance = 50_000_000_000;
    issue_asset(sandbox, &test, &asset, limit, initial_balance).await;

    // Create holder account for clawback
    let holder = new_account(sandbox, "holder");

    // Setup trustline for holder
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "change-trust",
            "--source",
            "holder",
            "--line",
            &asset,
        ])
        .assert()
        .success();

    // Authorize holder's trustline and enable clawback
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "set-trustline-flags",
            "--asset",
            &asset,
            "--trustor",
            &holder,
            "--set-authorize",
            "--source",
            "test1",
        ])
        .assert()
        .success();

    // Send some assets to the holder account
    let payment_amount = "10000000000"; // 1000 USDC
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "payment",
            "--destination",
            &holder,
            "--asset",
            &asset,
            "--amount",
            payment_amount,
            "--source",
            "test1",
        ])
        .assert()
        .success();

    // Test clawback command
    // this should succeed for the issuer
    let clawback_amount = "5000000000"; // 500 USDC
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "clawback",
            "--from",
            &holder,
            "--asset",
            &asset,
            "--amount",
            clawback_amount,
            "--source",
            "test1", // issuer should be able to clawback
        ])
        .assert()
        .success();

    // Verify holder's balance after clawback (should be 500 USDC: 1000 sent - 500 clawed back)
    let horizon_url = format!("http://localhost:8000/accounts/{}", holder);
    let response = reqwest::get(&horizon_url)
        .await
        .expect("Failed to fetch account from Horizon");
    let json: serde_json::Value = response
        .json()
        .await
        .expect("Failed to parse Horizon response");

    let final_balance = json["balances"]
        .as_array()
        .unwrap()
        .iter()
        .find(|balance| {
            balance["asset_code"].as_str() == Some("USDC")
                && balance["asset_issuer"].as_str() == Some(&issuer)
        })
        .expect("USDC balance not found after clawback")["balance"]
        .as_str()
        .unwrap()
        .parse::<f64>()
        .unwrap();

    assert_eq!(
        final_balance, 500.0,
        "Holder should have 500 USDC remaining after clawback (1000 sent - 500 clawed back)"
    );

    // Verify that a non-issuer cannot perform clawback
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "clawback",
            "--from",
            &holder,
            "--asset",
            &asset,
            "--amount",
            "1000000000", // 100 USDC
            "--source",
            "holder", // non-issuer should not be able to clawback
        ])
        .assert()
        .failure();
}
