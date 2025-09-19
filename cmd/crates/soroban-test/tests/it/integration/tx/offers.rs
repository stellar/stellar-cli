use soroban_test::TestEnv;

use crate::integration::util::{issue_asset, setup_accounts};

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
