use serde_json::Value;
use soroban_test::{AssertExt, TestEnv};

use crate::integration::{
    token::{deploy_sac, sac_balance, sac_id},
    util::{new_account, test_address},
};

/// Have `from` approve `spender` for `amount` of the token's SAC, so a
/// subsequent `transfer-from` has an allowance to draw on. `expiration_ledger`
/// is read from the live sequence with a comfortable buffer.
async fn approve(sandbox: &TestEnv, asset: &str, spender: &str, amount: i128) {
    let seq = sandbox.client().get_latest_ledger().await.unwrap().sequence;
    sandbox
        .new_assert_cmd("token")
        .args([
            "approve",
            "--id",
            asset,
            "--from",
            "test",
            "--spender",
            spender,
            "--amount",
            &amount.to_string(),
            "--expiration-ledger",
            &(seq + 1000).to_string(),
        ])
        .assert()
        .success();
}

#[tokio::test]
async fn transfer_from_moves_funds_using_allowance() {
    let sandbox = &TestEnv::new();
    let test = test_address(sandbox);
    let spender = new_account(sandbox, "spender");
    let recipient = new_account(sandbox, "recipient");

    deploy_sac(sandbox, "native", "test");
    let native_id = sac_id(sandbox, "native");

    let amount: i128 = 10_000_000;
    approve(sandbox, "native", &spender, amount).await;

    let owner_before = sac_balance(sandbox, &native_id, &test);
    let recipient_before = sac_balance(sandbox, &native_id, &recipient);

    // The spender (not the owner) signs, drawing on the allowance to move the
    // owner's funds to the recipient.
    let stdout = sandbox
        .new_assert_cmd("token")
        .args([
            "transfer-from",
            "--id",
            "native",
            "--spender",
            "spender",
            "--from",
            &test,
            "--to",
            &recipient,
            "--amount",
            &amount.to_string(),
            "--output",
            "json",
        ])
        .assert()
        .success()
        .stdout_as_str();
    let receipt: Value = serde_json::from_str(&stdout).unwrap();
    assert!(
        receipt["tx_hash"].as_str().is_some_and(|h| !h.is_empty()),
        "expected a non-empty tx_hash, got: {receipt}"
    );

    let owner_after = sac_balance(sandbox, &native_id, &test);
    let recipient_after = sac_balance(sandbox, &native_id, &recipient);
    assert_eq!(
        owner_after,
        owner_before - amount,
        "owner balance should decrease"
    );
    assert_eq!(
        recipient_after,
        recipient_before + amount,
        "recipient balance should increase"
    );
}

#[tokio::test]
async fn transfer_from_fails_when_sac_not_deployed() {
    let sandbox = &TestEnv::new();
    let issuer = new_account(sandbox, "issuer");
    let spender = new_account(sandbox, "spender");
    let recipient = new_account(sandbox, "recipient");
    let asset = format!("USDC:{issuer}");

    // No SAC deployed → structured deploy-pointer error with a typed discriminator.
    let stdout = sandbox
        .new_assert_cmd("token")
        .args([
            "transfer-from",
            "--id",
            &asset,
            "--spender",
            &spender,
            "--from",
            &issuer,
            "--to",
            &recipient,
            "--amount",
            "1",
            "--output",
            "json",
        ])
        .assert()
        .failure()
        .stdout_as_str();
    let value: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(
        value["error"]["type"], "sac_not_deployed",
        "expected a typed error, got: {stdout}"
    );
}

#[tokio::test]
async fn transfer_from_rejects_muxed_spender_with_clear_error() {
    let sandbox = &TestEnv::new();
    let test = test_address(sandbox);
    let recipient = new_account(sandbox, "recipient");

    // Muxed (M…) source accounts aren't supported by the invoke pipeline yet
    // (see #2645). The signer here is `--spender`, so the guard must name it.
    let muxed = "MA3D5KRYM6CB7OWQ6TWYRR3Z4T7GNZLKERYNZGGA5SOAOPIFY6YQGAAAAAAAAAPCICBKU";
    sandbox
        .new_assert_cmd("token")
        .args([
            "transfer-from",
            "--id",
            "native",
            "--spender",
            muxed,
            "--from",
            &test,
            "--to",
            &recipient,
            "--amount",
            "1",
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "muxed (M…) source accounts are not yet supported",
        ));
}

#[tokio::test]
async fn transfer_from_rejects_negative_amount_before_any_rpc() {
    let sandbox = &TestEnv::new();
    let test = test_address(sandbox);
    let recipient = new_account(sandbox, "recipient");

    // A negative amount is rejected at the clap layer, before any network call.
    sandbox
        .new_assert_cmd("token")
        .args([
            "transfer-from",
            "--id",
            "native",
            "--spender",
            "spender",
            "--from",
            &test,
            "--to",
            &recipient,
            // `=` form so clap reads `-1` as the value, not an unknown flag.
            "--amount=-1",
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains("amount must not be negative"));
}
