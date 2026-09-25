use serde_json::Value;
use soroban_test::{AssertExt, TestEnv};

use crate::integration::{
    token::{add_trustline, deploy_sac, issuer_pays, sac_balance, sac_id},
    util::{new_account, test_address},
};

/// Have `from` approve `spender` for `amount` of the token's SAC, so a
/// subsequent `burn-from` has an allowance to draw on. `expiration_ledger` is
/// read from the live sequence with a comfortable buffer.
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
async fn burn_from_destroys_funds_using_allowance() {
    let sandbox = &TestEnv::new();
    let test = test_address(sandbox);
    let issuer = new_account(sandbox, "issuer");
    let spender = new_account(sandbox, "spender");
    let asset = format!("USDC:{issuer}");

    // Burn is invalid on the native asset, so use an issued asset: the owner
    // (`test`) needs a trustline and a balance from the issuer to have something
    // to destroy.
    add_trustline(sandbox, "test", &asset);
    issuer_pays(sandbox, "issuer", &test, &asset, 1_000);
    deploy_sac(sandbox, &asset, "issuer");
    let contract_id = sac_id(sandbox, &asset);

    let amount: i128 = 400;
    approve(sandbox, &asset, &spender, amount).await;

    let before = sac_balance(sandbox, &contract_id, &test);

    // The spender (not the owner) signs, drawing on the allowance to destroy the
    // owner's tokens.
    let stdout = sandbox
        .new_assert_cmd("token")
        .args([
            "burn-from",
            "--id",
            &asset,
            "--spender",
            "spender",
            "--from",
            &test,
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
    assert_eq!(
        after,
        before - amount,
        "owner balance should decrease by amount"
    );
}

#[tokio::test]
async fn burn_from_fails_when_sac_not_deployed() {
    let sandbox = &TestEnv::new();
    let issuer = new_account(sandbox, "issuer");
    let spender = new_account(sandbox, "spender");
    let asset = format!("USDC:{issuer}");

    // No SAC deployed → structured deploy-pointer error with a typed discriminator.
    let stdout = sandbox
        .new_assert_cmd("token")
        .args([
            "burn-from",
            "--id",
            &asset,
            "--spender",
            &spender,
            "--from",
            &issuer,
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
async fn burn_from_rejects_muxed_spender_with_clear_error() {
    let sandbox = &TestEnv::new();
    let test = test_address(sandbox);

    // Muxed (M…) accounts aren't supported by the invoke pipeline yet (see
    // #2645); a muxed `--spender` is rejected up front.
    let muxed = "MA3D5KRYM6CB7OWQ6TWYRR3Z4T7GNZLKERYNZGGA5SOAOPIFY6YQGAAAAAAAAAPCICBKU";
    sandbox
        .new_assert_cmd("token")
        .args([
            "burn-from",
            "--id",
            "native",
            "--spender",
            muxed,
            "--from",
            &test,
            "--amount",
            "1",
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "muxed (M…) accounts are not yet supported",
        ));
}

#[tokio::test]
async fn burn_from_rejects_muxed_from_alias_with_clear_error() {
    let sandbox = &TestEnv::new();
    let spender = new_account(sandbox, "spender");

    // The host rejects a muxed (`M…`) owner mid-simulation, so an alias whose
    // stored key is muxed is rejected up front with a clear message instead of
    // failing opaquely.
    let muxed = "MA3D5KRYM6CB7OWQ6TWYRR3Z4T7GNZLKERYNZGGA5SOAOPIFY6YQGAAAAAAAAAPCICBKU";
    sandbox
        .new_assert_cmd("keys")
        .args(["add", "muxed-owner", "--public-key", muxed])
        .assert()
        .success();

    sandbox
        .new_assert_cmd("token")
        .args([
            "burn-from",
            "--id",
            "native",
            "--spender",
            &spender,
            "--from",
            "muxed-owner",
            "--amount",
            "1",
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "muxed (M…) accounts are not yet supported",
        ));
}

#[tokio::test]
async fn burn_from_rejects_muxed_from_strkey_with_clear_error() {
    let sandbox = &TestEnv::new();
    let spender = new_account(sandbox, "spender");

    // A literal `M…` owner is a resolved muxed address, which the host also
    // rejects; the command must reject it up front too, not just aliases.
    let muxed = "MA3D5KRYM6CB7OWQ6TWYRR3Z4T7GNZLKERYNZGGA5SOAOPIFY6YQGAAAAAAAAAPCICBKU";
    sandbox
        .new_assert_cmd("token")
        .args([
            "burn-from",
            "--id",
            "native",
            "--spender",
            &spender,
            "--from",
            muxed,
            "--amount",
            "1",
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "muxed (M…) accounts are not yet supported",
        ));
}

#[tokio::test]
async fn burn_from_rejects_negative_amount_before_any_rpc() {
    let sandbox = &TestEnv::new();
    let test = test_address(sandbox);

    // A negative amount is rejected at the clap layer, before any network call.
    sandbox
        .new_assert_cmd("token")
        .args([
            "burn-from",
            "--id",
            "native",
            "--spender",
            "spender",
            "--from",
            &test,
            // `=` form so clap reads `-1` as the value, not an unknown flag.
            "--amount=-1",
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains("amount must not be negative"));
}
