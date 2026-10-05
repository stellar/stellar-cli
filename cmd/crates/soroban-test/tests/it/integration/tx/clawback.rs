use soroban_test::TestEnv;

use crate::integration::util::{horizon_get_until, issue_asset, new_account, setup_accounts};

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
    let usdc_balance = |json: &serde_json::Value| {
        json["balances"].as_array()?.iter().find(|balance| {
            balance["asset_code"].as_str() == Some("USDC")
                && balance["asset_issuer"].as_str() == Some(&issuer)
        })?["balance"]
            .as_str()?
            .parse::<f64>()
            .ok()
    };
    let json = horizon_get_until(&horizon_url, |json| usdc_balance(json) == Some(500.0)).await;

    let final_balance = usdc_balance(&json).expect("USDC balance not found after clawback");

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
