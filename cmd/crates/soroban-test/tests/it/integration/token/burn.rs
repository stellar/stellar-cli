use serde_json::Value;
use soroban_test::{AssertExt, TestEnv};

use crate::integration::{
    token::{add_trustline, deploy_sac, issuer_pays, sac_balance, sac_id},
    util::{new_account, test_address},
};

#[tokio::test]
async fn burn_destroys_funds() {
    let sandbox = &TestEnv::new();
    let test = test_address(sandbox);
    let issuer = new_account(sandbox, "issuer");
    let asset = format!("USDC:{issuer}");

    // Burn is invalid on the native asset, so use an issued asset: the holder
    // (`test`) needs a trustline and a balance from the issuer to have something
    // to destroy.
    add_trustline(sandbox, "test", &asset);
    issuer_pays(sandbox, "issuer", &test, &asset, 1_000);
    deploy_sac(sandbox, &asset, "issuer");
    let contract_id = sac_id(sandbox, &asset);

    let amount: i128 = 400;
    let before = sac_balance(sandbox, &contract_id, &test);

    // `test` signs, destroying its own tokens.
    let stdout = sandbox
        .new_assert_cmd("token")
        .args([
            "burn",
            "--id",
            &asset,
            "--from",
            "test",
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

    let after = sac_balance(sandbox, &contract_id, &test);
    assert_eq!(after, before - amount, "balance should decrease by amount");
}

#[tokio::test]
async fn burn_fails_when_sac_not_deployed() {
    let sandbox = &TestEnv::new();
    let issuer = new_account(sandbox, "issuer");
    let asset = format!("USDC:{issuer}");

    // No SAC deployed → structured deploy-pointer error with a typed discriminator.
    let stdout = sandbox
        .new_assert_cmd("token")
        .args([
            "burn", "--id", &asset, "--from", &issuer, "--amount", "1", "--output", "json",
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
async fn burn_rejects_muxed_from_with_clear_error() {
    let sandbox = &TestEnv::new();

    // Muxed (M…) source accounts aren't supported by the invoke pipeline yet
    // (see #2645). The signer here is `--from`, so the guard must name it.
    let muxed = "MA3D5KRYM6CB7OWQ6TWYRR3Z4T7GNZLKERYNZGGA5SOAOPIFY6YQGAAAAAAAAAPCICBKU";
    sandbox
        .new_assert_cmd("token")
        .args(["burn", "--id", "native", "--from", muxed, "--amount", "1"])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "muxed (M…) source accounts are not yet supported",
        ));
}

#[tokio::test]
async fn burn_rejects_negative_amount_before_any_rpc() {
    let sandbox = &TestEnv::new();

    // A negative amount is rejected at the clap layer, before any network call.
    sandbox
        .new_assert_cmd("token")
        .args([
            "burn",
            "--id",
            "native",
            "--from",
            "test",
            // `=` form so clap reads `-1` as the value, not an unknown flag.
            "--amount=-1",
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains("amount must not be negative"));
}
