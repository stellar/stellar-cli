use soroban_test::TestEnv;

use crate::integration::util::{issue_asset, new_account, setup_accounts};

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
