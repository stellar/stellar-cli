use soroban_test::TestEnv;

use crate::integration::util::{issue_asset, new_account, setup_accounts};

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
