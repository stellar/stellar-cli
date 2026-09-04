use serde_json::Value;
use soroban_test::{AssertExt, TestEnv};

use crate::integration::{
    token::{deploy_sac, sac_id},
    util::{new_account, test_address},
};

/// Read the on-chain allowance `from` granted `spender` through the token's SAC.
fn sac_allowance(sandbox: &TestEnv, contract_id: &str, from: &str, spender: &str) -> i128 {
    let stdout = sandbox
        .new_assert_cmd("contract")
        .args([
            "invoke",
            "--id",
            contract_id,
            "--source-account",
            "test",
            "--",
            "allowance",
            "--from",
            from,
            "--spender",
            spender,
        ])
        .assert()
        .success()
        .stdout_as_str();
    stdout.trim().trim_matches('"').parse().unwrap()
}

#[tokio::test]
async fn approve_sets_allowance_and_returns_receipt() {
    let sandbox = &TestEnv::new();
    let test = test_address(sandbox);
    let issuer = new_account(sandbox, "issuer");
    let spender = new_account(sandbox, "spender");
    let asset = format!("USDC:{issuer}");
    deploy_sac(sandbox, &asset, "issuer");

    // `expiration_ledger` must be at or beyond the current ledger for a positive
    // allowance; read the live sequence and leave a comfortable buffer.
    let seq = sandbox.client().get_latest_ledger().await.unwrap().sequence;
    let expiration = (seq + 1000).to_string();

    let stdout = sandbox
        .new_assert_cmd("token")
        .args([
            "approve",
            "--id",
            &asset,
            "--from",
            "test",
            "--spender",
            &spender,
            "--amount",
            "5000000",
            "--expiration-ledger",
            &expiration,
            "--output",
            "json",
        ])
        .assert()
        .success()
        .stdout_as_str();
    let receipt: Value = serde_json::from_str(&stdout).unwrap();
    assert!(
        receipt["tx_hash"].as_str().is_some(),
        "expected a tx hash, got: {receipt}"
    );

    // The allowance is now readable on-chain.
    let sac = sac_id(sandbox, &asset);
    assert_eq!(
        sac_allowance(sandbox, &sac, &test, &spender),
        5_000_000,
        "expected the approved allowance on-chain"
    );
}

#[tokio::test]
async fn approve_fails_when_sac_not_deployed() {
    let sandbox = &TestEnv::new();
    let issuer = new_account(sandbox, "issuer");
    let spender = new_account(sandbox, "spender");
    let asset = format!("USDC:{issuer}");

    // No SAC deployed → structured deploy-pointer error with a typed discriminator.
    let stdout = sandbox
        .new_assert_cmd("token")
        .args([
            "approve",
            "--id",
            &asset,
            "--from",
            "test",
            "--spender",
            &spender,
            "--amount",
            "1",
            "--expiration-ledger",
            "9999999",
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
async fn approve_rejects_negative_amount_before_any_rpc() {
    let sandbox = &TestEnv::new();
    let spender = new_account(sandbox, "spender");

    // A negative allowance is rejected at the CLI layer, before any network call.
    sandbox
        .new_assert_cmd("token")
        .args([
            "approve",
            "--id",
            "native",
            "--from",
            "test",
            "--spender",
            &spender,
            // `=` form so clap reads `-1` as the value, not an unknown flag.
            "--amount=-1",
            "--expiration-ledger",
            "9999999",
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains("must not be negative"));
}
