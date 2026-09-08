use serde_json::Value;
use soroban_test::{AssertExt, TestEnv};

use crate::integration::{
    token::{add_trustline, deploy_sac, sac_balance, sac_id},
    util::{new_account, test_address},
};

#[tokio::test]
async fn mint_credits_recipient_and_returns_receipt() {
    let sandbox = &TestEnv::new();
    let test = test_address(sandbox);
    let issuer = new_account(sandbox, "issuer");
    let asset = format!("USDC:{issuer}");

    // The SAC admin is the asset issuer. Minting a classic asset to an account
    // requires the recipient to hold a trustline.
    add_trustline(sandbox, "test", &asset);
    deploy_sac(sandbox, &asset, "issuer");

    let stdout = sandbox
        .new_assert_cmd("token")
        .args([
            "mint", "--id", &asset, "--admin", "issuer", "--to", &test, "--amount", "7500000",
            "--output", "json",
        ])
        .assert()
        .success()
        .stdout_as_str();
    let receipt: Value = serde_json::from_str(&stdout).unwrap();
    assert!(
        receipt["tx_hash"].as_str().is_some(),
        "expected a tx hash, got: {receipt}"
    );

    // The minted balance is now readable on-chain.
    let sac = sac_id(sandbox, &asset);
    assert_eq!(
        sac_balance(sandbox, &sac, &test),
        7_500_000,
        "expected the minted balance on-chain"
    );
}

#[tokio::test]
async fn mint_fails_when_sac_not_deployed() {
    let sandbox = &TestEnv::new();
    let test = test_address(sandbox);
    let issuer = new_account(sandbox, "issuer");
    let asset = format!("USDC:{issuer}");

    // No SAC deployed → structured deploy-pointer error with a typed discriminator.
    let stdout = sandbox
        .new_assert_cmd("token")
        .args([
            "mint", "--id", &asset, "--admin", "issuer", "--to", &test, "--amount", "1",
            "--output", "json",
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
async fn mint_rejects_negative_amount_before_any_rpc() {
    let sandbox = &TestEnv::new();
    let test = test_address(sandbox);

    // A negative mint is rejected at the CLI layer, before any network call.
    // `=` form so clap reads `-1` as the value, not an unknown flag.
    sandbox
        .new_assert_cmd("token")
        .args([
            "mint",
            "--id",
            "native",
            "--admin",
            "test",
            "--to",
            &test,
            "--amount=-1",
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains("must not be negative"));
}
