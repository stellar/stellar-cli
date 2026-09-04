use serde_json::Value;
use soroban_test::{AssertExt, TestEnv};

use crate::integration::{
    token::{add_trustline, deploy_sac, issuer_pays, sac_balance, sac_id},
    util::{new_account, test_address},
};

/// Enable the clawback flag on `issuer`, so trustlines created afterwards are
/// clawback-enabled. `AUTH_CLAWBACK_ENABLED` requires `AUTH_REVOCABLE`, so set
/// both together.
fn enable_clawback(sandbox: &TestEnv, issuer: &str) {
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "set-options",
            "--set-revocable",
            "--set-clawback-enabled",
            "--source",
            issuer,
        ])
        .assert()
        .success();
}

#[tokio::test]
async fn clawback_removes_balance_and_returns_receipt() {
    let sandbox = &TestEnv::new();
    let test = test_address(sandbox);
    let issuer = new_account(sandbox, "issuer");
    let asset = format!("USDC:{issuer}");

    // Clawback requires the issuer to enable the flag *before* the holder's
    // trustline exists, so the trustline is created clawback-enabled.
    enable_clawback(sandbox, "issuer");
    add_trustline(sandbox, "test", &asset);
    deploy_sac(sandbox, &asset, "issuer");
    issuer_pays(sandbox, "issuer", &test, &asset, 10_000_000);

    let stdout = sandbox
        .new_assert_cmd("token")
        .args([
            "clawback", "--id", &asset, "--admin", "issuer", "--from", &test, "--amount",
            "4000000", "--output", "json",
        ])
        .assert()
        .success()
        .stdout_as_str();
    let receipt: Value = serde_json::from_str(&stdout).unwrap();
    assert!(
        receipt["tx_hash"].as_str().is_some(),
        "expected a tx hash, got: {receipt}"
    );

    // 10_000_000 minted − 4_000_000 clawed back = 6_000_000 remaining.
    let sac = sac_id(sandbox, &asset);
    assert_eq!(
        sac_balance(sandbox, &sac, &test),
        6_000_000,
        "expected the remaining balance after clawback"
    );
}

#[tokio::test]
async fn clawback_fails_when_sac_not_deployed() {
    let sandbox = &TestEnv::new();
    let test = test_address(sandbox);
    let issuer = new_account(sandbox, "issuer");
    let asset = format!("USDC:{issuer}");

    // No SAC deployed → structured deploy-pointer error with a typed discriminator.
    let stdout = sandbox
        .new_assert_cmd("token")
        .args([
            "clawback", "--id", &asset, "--admin", "issuer", "--from", &test, "--amount", "1",
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
async fn clawback_rejects_muxed_source_with_clear_error() {
    let sandbox = &TestEnv::new();
    let holder = new_account(sandbox, "holder");

    // Muxed (M…) source accounts aren't supported by the invoke pipeline yet
    // (see #2645). Until then the command must reject them up front with a clear
    // message rather than a raw strkey decode error deep in the pipeline.
    let muxed = "MA3D5KRYM6CB7OWQ6TWYRR3Z4T7GNZLKERYNZGGA5SOAOPIFY6YQGAAAAAAAAAPCICBKU";
    sandbox
        .new_assert_cmd("token")
        .args([
            "clawback", "--id", "native", "--admin", muxed, "--from", &holder, "--amount", "1",
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "muxed (M…) source accounts are not yet supported",
        ));
}

#[tokio::test]
async fn clawback_rejects_negative_amount_before_any_rpc() {
    let sandbox = &TestEnv::new();
    let test = test_address(sandbox);

    // A negative clawback is rejected at the CLI layer, before any network call.
    // `=` form so clap reads `-1` as the value, not an unknown flag.
    sandbox
        .new_assert_cmd("token")
        .args([
            "clawback",
            "--id",
            "native",
            "--admin",
            "test",
            "--from",
            &test,
            "--amount=-1",
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains("must not be negative"));
}
